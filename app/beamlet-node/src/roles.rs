//! 静态 Authority/Worker 进程, 只在应用边界交付已有授权和证据

use std::{sync::Arc, time::Duration};

use anyhow::{Context, Result, bail, ensure};
use beamlet::{
    Authority, Worker,
    attempt_delivery::{
        self, Event as AttemptEvent, Outbox as AttemptOutbox, Ticket, Transmission,
    },
    id::{OperationId, ReportId, RoleInstanceId, SubmissionKey},
    lifecycle::Shutdown,
    memory::{Admission, StateError},
    model::{Delivery, DispatchAttempt, ExecutionResult, ImpactStatus, Message, SubmitReport},
    report_delivery::{self, Event as ReportEvent, Outbox as ReportOutbox},
};
use beamlet_message_zenoh::{Limits, Transport};
use beamlet_protocol_demo::{
    attempt_receipt::{self, AttemptAccepted},
    hello::{self, Dispatch, Response, ResponseMessage, Task},
    protocol::Limits as ProtocolLimits,
    query,
    receipt::{self, ReportRetained},
    submission::{Submission, SubmissionReply},
};
use tokio::{
    task::JoinSet,
    time::{Instant, MissedTickBehavior, interval, timeout},
};
use uuid::Uuid;

use crate::shutdown;

pub(crate) const NODE_INBOX_PREFIX: &str = "beamlet/v0/node/inbox/";
const SEND_TIMEOUT: Duration = Duration::from_secs(10);
const SEND_BUDGET: Duration = Duration::from_millis(500);

type HelloAuthority = Authority<Task, String, String>;

// 留出 Console 的 10 秒等待预算, 只结束本次观察, 不取消 Worker
const REPLY_WAIT: Duration = Duration::from_secs(5);

struct Waiter {
    operation: OperationId,
    console: String,
    binding: SubmissionReply,
    deadline: Instant,
}

enum SendPurpose {
    Notice,
    Dispatch(Ticket),
    Reply,
    Receipt,
    SubmissionReceipt,
    AttemptReceipt,
    Report(report_delivery::Ticket),
}

struct Outgoing {
    destination: String,
    bytes: Vec<u8>,
    purpose: SendPurpose,
}

type Sends = JoinSet<(SendPurpose, Result<()>)>;

/// 启动只接纳请求和收录证据的静态 Authority 进程
///
/// # 错误
/// 配置, 会话, 驱动状态或后台任务无法继续时返回错误
pub async fn run_authority(
    endpoint: &str,
    instance: RoleInstanceId,
    workers: Vec<RoleInstanceId>,
) -> Result<()> {
    let transport = Arc::new(
        Transport::listen(
            endpoint,
            &format!("{NODE_INBOX_PREFIX}*"),
            Limits::default(),
        )
        .await?,
    );
    serve_authority(
        transport,
        instance,
        workers,
        None,
        NODE_INBOX_PREFIX.to_owned(),
    )
    .await
}

pub(crate) async fn serve_authority(
    transport: Arc<Transport>,
    instance: RoleInstanceId,
    workers: Vec<RoleInstanceId>,
    local_worker: Option<Worker<Task, String, String>>,
    request_prefix: String,
) -> Result<()> {
    shutdown::standalone(move |control| {
        serve_authority_pair(
            transport,
            None,
            AuthoritySetup {
                instance,
                workers,
                local_worker,
                request_prefix,
            },
            control,
        )
    })
    .await
}

pub(crate) struct AuthoritySetup {
    pub instance: RoleInstanceId,
    pub workers: Vec<RoleInstanceId>,
    pub local_worker: Option<Worker<Task, String, String>>,
    pub request_prefix: String,
}

pub(crate) async fn serve_authority_pair(
    transport: Arc<Transport>,
    report_transport: Option<Transport>,
    setup: AuthoritySetup,
    shutdown: Shutdown,
) -> Result<()> {
    serve_authority_with_executor(transport, report_transport, setup, shutdown, execute_hello).await
}

async fn serve_authority_with_executor<F, Fut>(
    transport: Arc<Transport>,
    report_transport: Option<Transport>,
    setup: AuthoritySetup,
    mut shutdown: Shutdown,
    execute: F,
) -> Result<()>
where
    F: Fn(DispatchAttempt<Task>) -> Fut + Send + Sync,
    Fut: std::future::Future<Output = SubmitReport<String, String>> + Send + 'static,
{
    let AuthoritySetup {
        instance,
        workers,
        mut local_worker,
        request_prefix,
    } = setup;
    let limits = beamlet::memory::Limits::default();
    let mut authority = HelloAuthority::new(instance, workers, limits)?;
    let protocol = ProtocolLimits::default();
    let reports = report_inbox(instance);
    let receipt_inbox = attempt_receipt::inbox(instance);
    let receipts = transport.subscribe(&receipt_inbox).await?;
    let query_prefix = query::prefix(instance);
    let queries = transport.subscribe(&format!("{query_prefix}*")).await?;
    let mut attempts = AttemptOutbox::new(
        instance,
        attempt_delivery::Limits {
            max_records: limits.max_records,
            max_pending: limits.queue_capacity,
        },
    )?;
    let mut waiters: Vec<Waiter> = Vec::new();
    let mut sends = Sends::new();
    let mut pending = std::collections::HashSet::new();
    let mut draining = false;
    let mut tick = interval(Duration::from_millis(100));
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    println!("Authority 已就绪: 实例={instance}");
    println!(
        "内存记录上限={}; 等待回复与发送任务各自上限={}",
        limits.max_records, limits.queue_capacity
    );

    loop {
        if shutdown.is_draining() && !draining {
            draining = true;
            println!(
                "Authority 进入 Draining: 实例={instance}; 未报告 Operation={}",
                pending.len()
            );
        }
        flush_replies(
            &mut authority,
            &mut waiters,
            &mut sends,
            &transport,
            &protocol,
            limits.queue_capacity,
        )?;
        while sends.len() < limits.queue_capacity {
            let Some(event) = attempts.next_event() else {
                break;
            };
            match event {
                AttemptEvent::Send(transmission) => {
                    queue_attempt(&mut sends, &transport, transmission)
                }
                AttemptEvent::Exhausted(progress) => eprintln!(
                    "Attempt 交付预算耗尽: Operation={}; 接纳未确认, 不改派或重新授权",
                    progress.binding.operation_id
                ),
            }
        }
        tokio::select! {
            _ = shutdown.requested(), if !draining => {}
            received = receipts.recv_message() => {
                match received {
                    Ok(message) => {
                        let result = (|| {
                            let receipt = AttemptAccepted::from_json(&message.payload)?;
                            ensure!(message.key_expr == receipt_inbox && message.key_expr == receipt.topic(), "Attempt 回执实际路由不符");
                            if attempts.acknowledge(receipt.binding())? {
                                println!("Authority 已确认 Worker 接纳: Operation={} Attempt={}; 不表示执行已开始或完成", receipt.operation_id, receipt.attempt_id);
                            }
                            Ok::<_,anyhow::Error>(())
                        })();
                        if let Err(error) = result { eprintln!("已忽略 Attempt 接纳回执: {error}"); }
                    }
                    Err(error @ (beamlet_message_zenoh::Error::UnexpectedDelete | beamlet_message_zenoh::Error::MessageTooLarge)) => eprintln!("已忽略 Attempt 接纳回执: {error}"),
                    Err(error) => return Err(error).context("接收 Attempt 回执失败"),
                }
            }
            received = async {
                if let Some(reports) = &report_transport {
                    tokio::select! {
                        received = transport.recv_message() => received,
                        received = reports.recv_message() => received,
                        received = queries.recv_message() => received,
                    }
                } else {
                    tokio::select! {
                        received = transport.recv_message() => received,
                        received = queries.recv_message() => received,
                    }
                }
            } => {
                let message = match received {
                    Ok(message) => message,
                    Err(error @ (beamlet_message_zenoh::Error::UnexpectedDelete | beamlet_message_zenoh::Error::MessageTooLarge)) => { eprintln!("已忽略非法提交或 Report: {error}"); continue; }
                    Err(error) => return Err(error).context("Authority 接收消息失败"),
                };
                if message.key_expr == reports {
                    let delivery = match hello::decode_report(&message.payload, &protocol) {
                        Ok(delivery) => delivery,
                        Err(error) => {
                            eprintln!("已忽略无效 Report: {error}");
                            continue;
                        }
                    };
                    let operation = match &delivery.message {
                        Message::SubmitReport(report) => report.operation_id,
                        _ => unreachable!("Report 解码只能返回 Report"),
                    };
                    let receipt = ReportRetained::for_report(&delivery)?;
                    match authority.receive_report(delivery) {
                        Ok(disposition) => {
                            pending.remove(&operation);
                            attempts.reported(&authority, receipt.report_id.as_u128().into())?;
                            println!("Authority 已接收 Report: {disposition}; 停止原授权重投");
                            if sends.len() < limits.queue_capacity {
                                queue_send(&mut sends, &transport, Outgoing {
                                    destination: receipt.topic(),
                                    bytes: receipt.to_json()?,
                                    purpose: SendPurpose::Receipt,
                                });
                            } else {
                                eprintln!("Report 已收录, 回执发送槽位已满: Report={}; 等待原证据重投, 不回滚收录", receipt.report_id);
                            }
                            queue_notice(&authority, receipt.report_id.as_u128().into(), &mut sends, &transport, &protocol, limits.queue_capacity);
                        },
                        Err(error) => eprintln!("Report 未收录: {error}"),
                    }
                    continue;
                }
                let (console, is_query) = if let Some(console) = message.key_expr.strip_prefix(&query_prefix) {
                    (console, true)
                } else if let Some(console) = message.key_expr.strip_prefix(&request_prefix) {
                    (console, false)
                } else { continue; };
                if console.len() != 32 || !console.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                    eprintln!("已忽略无效 Console 路由: {console:?}");
                    continue;
                }
                if is_query {
                    if sends.len() >= limits.queue_capacity {
                        eprintln!("查询回复发送槽位已满; 未修改账本");
                        continue;
                    }
                    match query::respond(&authority, &message.payload, &protocol) {
                        Ok(bytes) => queue_send(&mut sends, &transport, Outgoing {
                            destination: format!("beamlet/v0/console/{console}/inbox"),
                            bytes,
                            purpose: SendPurpose::Reply,
                        }),
                        Err(error) => eprintln!("已忽略无效查询: {error}"),
                    }
                    continue;
                }
                let request = match Submission::from_json(&message.payload, &protocol) {
                    Ok(request) => request,
                    Err(error) => {
                        eprintln!("提交无效, 本次未处理: {error}");
                        continue;
                    }
                };
                if sends.len() + 2 > limits.queue_capacity {
                    eprintln!("提交发送槽位已满; 本次未处理, 不创建身份");
                    continue;
                }
                let key = request.submission_key;
                let submission_key = SubmissionKey::from_u128(key.as_u128());
                let existing = match authority.check_submission(submission_key, &request.task) {
                    Ok(existing) => existing,
                    Err(error) => {
                        queue_submission(&mut sends, &transport, console, SubmissionReply::rejected(key, Uuid::from_u128(instance.as_u128()), error.to_string()), &protocol)?;
                        continue;
                    }
                };
                let rejection = if existing.is_none() && shutdown.is_draining() {
                    Some("NodeShutdown: 新提交未接纳")
                } else if existing.is_none() && !attempts.has_capacity() {
                    Some("Attempt 交付槽位已满; 新提交未接纳")
                } else { None };
                if let Some(error) = rejection {
                    queue_submission(&mut sends, &transport, console, SubmissionReply::rejected(key, Uuid::from_u128(instance.as_u128()), error.to_owned()), &protocol)?;
                    continue;
                }
                let operation = existing.map(|view| {
                    let Message::DispatchAttempt(dispatch) = &view.dispatch.message else {
                        unreachable!("Authority 只能保留 Attempt 信封");
                    };
                    dispatch.operation.operation_id
                });
                let adopted = existing.and_then(|view| view.adopted_report);
                let already_waiting = operation.is_some_and(|operation| waiters.iter().any(|waiter| waiter.operation == operation && waiter.console == console));
                if adopted.is_none() && !already_waiting && waiters.len() >= limits.queue_capacity {
                    if let Some(existing) = existing {
                        let operation = operation.expect("已接纳执行必须包含 Operation");
                        let binding = SubmissionReply::accepted(key, &existing.dispatch)?;
                        queue_submission(&mut sends, &transport, console, binding.clone(), &protocol)?;
                        queue_reply(&mut sends, &transport, console, &binding, Response::new(ResponseMessage::Pending { operation_id: Uuid::from_u128(operation.as_u128()) }), &protocol)?;
                    } else {
                        queue_submission(&mut sends, &transport, console, SubmissionReply::rejected(key, Uuid::from_u128(instance.as_u128()), "等待回复记录已满; 新提交未接纳".to_owned()), &protocol)?;
                    }
                    continue;
                }
                let submission = match authority.submit_keyed(submission_key, request.task, || {
                    (Uuid::new_v4().as_u128().into(), Uuid::new_v4().as_u128().into())
                }) {
                    Ok(submission) => submission,
                    Err(error @ (StateError::Capacity | StateError::SubmissionConflict | StateError::OperationConflict | StateError::AttemptConflict)) => {
                        queue_submission(&mut sends, &transport, console, SubmissionReply::rejected(key, Uuid::from_u128(instance.as_u128()), error.to_string()), &protocol)?;
                        continue;
                    }
                    Err(error) => return Err(error.into()),
                };
                let Message::DispatchAttempt(dispatch) = &submission.dispatch.message else { bail!("接纳结果缺少 Attempt"); };
                let operation = dispatch.operation.operation_id;
                let binding = SubmissionReply::accepted(key, &submission.dispatch)?;
                queue_submission(&mut sends, &transport, console, binding.clone(), &protocol)?;
                println!("Authority 已接纳: SubmissionKey={key} Operation={operation} Worker={} 接纳结果={}", submission.dispatch.target_instance_id, submission.admission);
                if let Some(report_id) = adopted {
                    let report = authority.report(report_id).context("已采用的 Report 缺失")?;
                    queue_reply(&mut sends, &transport, console, &binding, response_for(report, operation)?, &protocol)?;
                    continue;
                }
                if !already_waiting {
                    waiters.push(Waiter { operation, console: console.to_owned(), binding, deadline: Instant::now() + REPLY_WAIT });
                }
                if submission.admission == Admission::New {
                    pending.insert(operation);
                    if local_worker.as_ref().is_some_and(|worker| worker.contains(submission.dispatch.target_instance_id)) {
                        // The local library driver picks up the retained authorization independently.
                        continue;
                    }
                    attempts.retain(&submission.dispatch, |dispatch| Dispatch::from_delivery(dispatch)?.to_json(&protocol))?;
                }
            }
            completed = async {
                if let Some(worker) = &mut local_worker
                    && let Some(report) = beamlet::local_delivery::next_report(&mut authority, worker, &execute).await? {
                    return Ok::<_, beamlet::LocalExecutionError>(report);
                }
                std::future::pending().await
            } => {
                let report = completed?;
                let Message::SubmitReport(evidence) = &report.message else { unreachable!("本地驱动只返回 Report"); };
                pending.remove(&evidence.operation_id);
                queue_notice(&authority, evidence.report_id, &mut sends, &transport, &protocol, limits.queue_capacity);
                println!("本地 Report 收录已确认: Report={}; 类型化交付", evidence.report_id);
            }
            completed = sends.join_next(), if !sends.is_empty() => {
                let (purpose, result) = completed.context("发送完成记录缺失")?.context("发送任务失败")?;
                match (purpose, result) {
                    (SendPurpose::Notice, _) => {},
                    (SendPurpose::Dispatch(ticket), Ok(())) => {
                        let operation = ticket.binding().operation_id;
                        attempts.sent(ticket);
                        authority.mark_dispatched(operation)?;
                        println!("Authority 已发出授权: Operation={operation}; 本次发送完成本身不证明 Worker 接纳");
                    }
                    (SendPurpose::Dispatch(ticket), Err(error)) => {
                        let operation = ticket.binding().operation_id;
                        attempts.sent(ticket);
                        eprintln!("授权本次发送未确认: Operation={operation}; {error}; 仅在原预算内重投, 不创建新授权");
                    },
                    (SendPurpose::Reply, Err(error)) => eprintln!("Console 回复未确认: {error}; 证据已保留, 不重试"),
                    (SendPurpose::Reply, Ok(())) => {}
                    (SendPurpose::Receipt, Ok(())) => println!("Report 收录回执已发出; 不等待回执的回执, 不表示持久化"),
                    (SendPurpose::Receipt, Err(error)) => eprintln!("Report 收录回执发送未确认: {error}; 原证据仍已保留, 合法重复可再次回复"),
                    (SendPurpose::SubmissionReceipt, Err(error)) => eprintln!("Operation 回执发送未确认: {error}; 接纳映射保留, 原 key 重投可再次回复"),
                    (SendPurpose::SubmissionReceipt, Ok(())) => {},
                    (SendPurpose::AttemptReceipt, _) => bail!("Authority 不应发送 Attempt 接纳回执"),
                    (SendPurpose::Report(_), _) => bail!("Authority 不应包含 Worker Report 发送任务"),
                }
            }
            _ = tick.tick() => {
                if draining && pending.is_empty() && waiters.is_empty() && sends.is_empty() {
                    println!("Authority Draining 完成: 实例={instance}; 已接纳 Operation 均已有 Report, 发送尝试已结束; 不保证跨重启保留");
                    return Ok(());
                }
            }
        }
    }
}

fn flush_replies(
    authority: &mut HelloAuthority,
    waiters: &mut Vec<Waiter>,
    sends: &mut Sends,
    transport: &Arc<Transport>,
    protocol: &ProtocolLimits,
    capacity: usize,
) -> Result<()> {
    while sends.len() < capacity {
        let now = Instant::now();
        let ready = waiters.iter().position(|waiter| {
            authority
                .operation(waiter.operation)
                .is_some_and(|view| view.adopted_report.is_some())
                || waiter.deadline <= now
        });
        let Some(index) = ready else { break };
        let waiter = waiters.swap_remove(index);
        let report_id = authority
            .operation(waiter.operation)
            .and_then(|view| view.adopted_report);
        let response = if let Some(report_id) = report_id {
            let report = authority
                .report(report_id)
                .context("已采用的 Report 缺失")?;
            response_for(report, waiter.operation)?
        } else {
            authority.mark_wait_timeout(waiter.operation)?;
            println!(
                "Authority 待定: Operation={}; 不自动重试或创建新授权",
                waiter.operation
            );
            Response::new(ResponseMessage::Pending {
                operation_id: Uuid::from_u128(waiter.operation.as_u128()),
            })
        };
        queue_reply(
            sends,
            transport,
            &waiter.console,
            &waiter.binding,
            response,
            protocol,
        )?;
    }
    Ok(())
}

fn queue_reply(
    sends: &mut Sends,
    transport: &Arc<Transport>,
    console: &str,
    binding: &SubmissionReply,
    response: Response,
    protocol: &ProtocolLimits,
) -> Result<()> {
    queue_submission(
        sends,
        transport,
        console,
        binding.with_response(response),
        protocol,
    )
}

fn queue_submission(
    sends: &mut Sends,
    transport: &Arc<Transport>,
    console: &str,
    reply: SubmissionReply,
    protocol: &ProtocolLimits,
) -> Result<()> {
    queue_send(
        sends,
        transport,
        Outgoing {
            destination: format!("beamlet/v0/console/{console}/inbox"),
            bytes: reply.to_json(protocol)?,
            purpose: if reply.response.is_none() {
                SendPurpose::SubmissionReceipt
            } else {
                SendPurpose::Reply
            },
        },
    );
    Ok(())
}

fn queue_send(sends: &mut Sends, transport: &Arc<Transport>, outgoing: Outgoing) {
    // 有界发送任务共享同一会话, 不持有 Authority 状态或锁
    let transport = Arc::clone(transport);
    sends.spawn(async move {
        let budget = match &outgoing.purpose {
            SendPurpose::Notice
            | SendPurpose::SubmissionReceipt
            | SendPurpose::Receipt
            | SendPurpose::AttemptReceipt => SEND_BUDGET,
            _ => SEND_TIMEOUT,
        };
        let result = send(&transport, &outgoing.destination, outgoing.bytes, budget).await;
        (outgoing.purpose, result)
    });
}

fn queue_attempt(sends: &mut Sends, transport: &Arc<Transport>, transmission: Transmission) {
    let ticket = transmission.ticket();
    println!(
        "Authority 重用原授权发送: Operation={} Attempt={}; 发送尝试次数={}/{}",
        ticket.binding().operation_id,
        ticket.binding().attempt_id,
        ticket.number(),
        attempt_delivery::MAX_SENDS
    );
    let transport = Arc::clone(transport);
    sends.spawn(async move {
        let result = transmission
            .send(|worker, bytes| async move {
                let sender = transport.sender(&worker_inbox(worker)).await?;
                sender.wait_for_receiver().await?;
                sender.send(bytes).await
            })
            .await;
        (
            SendPurpose::Dispatch(ticket),
            result.map_err(anyhow::Error::from),
        )
    });
}

fn queue_report(
    sends: &mut Sends,
    transport: &Arc<Transport>,
    transmission: report_delivery::Transmission,
) {
    let ticket = transmission.ticket();
    println!(
        "Worker 发送原 Report: Report={}; 发送尝试次数={}/{}; 尚未确认 Authority 收录",
        ticket.binding().report_id,
        ticket.number(),
        report_delivery::MAX_SENDS
    );
    let transport = Arc::clone(transport);
    sends.spawn(async move {
        let result = transmission
            .send(|authority, bytes| async move {
                let sender = transport.sender(&report_inbox(authority)).await?;
                sender.wait_for_receiver().await?;
                sender.send(bytes).await
            })
            .await;
        (
            SendPurpose::Report(ticket),
            result.map_err(anyhow::Error::from),
        )
    });
}

fn queue_notice(
    authority: &HelloAuthority,
    report: ReportId,
    sends: &mut Sends,
    transport: &Arc<Transport>,
    protocol: &ProtocolLimits,
    capacity: usize,
) {
    if sends.len() >= capacity {
        return;
    }
    let Some(notice) = authority.report_notice(report) else {
        return;
    };
    let destination = query::notice_topic(notice.operation.binding.authority_instance_id);
    match query::encode_notice(notice, protocol) {
        Ok(bytes) => queue_send(
            sends,
            transport,
            Outgoing {
                destination,
                bytes,
                purpose: SendPurpose::Notice,
            },
        ),
        Err(error) => eprintln!("结果通知编码失败: {error}; 可通过查询读取已保留事实"),
    }
}

async fn send(
    transport: &Transport,
    destination: &str,
    bytes: Vec<u8>,
    budget: Duration,
) -> Result<()> {
    timeout(budget, async {
        let sender = transport.sender(destination).await?;
        sender.wait_for_receiver().await?;
        sender.send(bytes).await?;
        Ok::<_, anyhow::Error>(())
    })
    .await
    .context("消息发送超时")?
}

/// 启动只执行绑定 Authority 授权的静态 Worker 进程
///
/// # 错误
/// 配置, 连接, 接收或内部执行驱动失败时返回错误
pub async fn run_worker(
    endpoint: &str,
    instance: RoleInstanceId,
    authority: RoleInstanceId,
) -> Result<()> {
    let inbox = worker_inbox(instance);
    let transport = timeout(
        SEND_TIMEOUT,
        Transport::connect(endpoint, &inbox, Limits::default()),
    )
    .await
    .context("Worker 连接超时")??;
    let receipts = transport.subscribe(&receipt::inbox(instance)).await?;
    shutdown::standalone(move |control| {
        serve_worker(transport, receipts, instance, authority, control)
    })
    .await
}

pub(crate) async fn serve_worker(
    transport: Transport,
    receipts: Transport,
    instance: RoleInstanceId,
    authority: RoleInstanceId,
    mut shutdown: Shutdown,
) -> Result<()> {
    let limits = beamlet::memory::Limits::default();
    let mut worker = Worker::<Task, String, String>::new(authority, vec![instance], limits)?;
    let transport = Arc::new(transport);
    let protocol = ProtocolLimits::default();
    let inbox = worker_inbox(instance);
    let receipt_inbox = receipt::inbox(instance);
    let mut deliveries = ReportOutbox::new(
        instance,
        authority,
        report_delivery::Limits {
            max_records: limits.max_records,
            max_pending: limits.queue_capacity,
        },
    )?;
    let mut sends = Sends::new();
    let mut draining = false;
    let mut tick = interval(Duration::from_millis(100));
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    println!("Worker 已就绪: 实例={instance} Authority={authority}");
    loop {
        if shutdown.is_draining() && !draining {
            draining = true;
            println!(
                "Worker 进入 Draining: 实例={instance}; 已接纳未完成 Attempt={}; 未确认 Report={}",
                worker.pending(),
                deliveries.unconfirmed()
            );
        }
        while sends.len() < limits.queue_capacity {
            let Some(event) = deliveries.next_event() else {
                break;
            };
            match event {
                ReportEvent::Send(transmission) => {
                    queue_report(&mut sends, &transport, transmission)
                }
                ReportEvent::Exhausted(progress) => eprintln!(
                    "Report 交付预算耗尽: Report={}; 发送尝试次数={}; 收录未确认, 保留原证据, 不重跑",
                    progress.binding.report_id, progress.sends
                ),
            }
        }
        tokio::select! {
            biased;
            _ = shutdown.requested(), if !draining => {}
            completed = sends.join_next(), if !sends.is_empty() => {
                let (purpose, result) = completed.context("Report 发送完成记录缺失")?.context("Report 发送任务失败")?;
                match purpose {
                    SendPurpose::Report(ticket) => {
                        let report = ticket.binding().report_id;
                        deliveries.sent(ticket);
                        if let Err(error) = result { eprintln!("Report 本次发送未确认: Report={report}; {error}; 仅在原交付预算内重投, 不重跑"); }
                    }
                    SendPurpose::AttemptReceipt => {
                        if let Err(error) = result { eprintln!("Attempt 回执发送未确认: {error}; 合法重复授权可再次回复"); }
                    }
                    _ => bail!("Worker 发送任务种类错误"),
                }
            }
            completed = worker.next_report(execute_hello), if worker.pending() > 0 && sends.len() < limits.queue_capacity => {
                let report = completed?.context("Worker 完成记录缺失")?;
                let Message::SubmitReport(evidence) = &report.message else { bail!("Worker 完成结果不包含 Report"); };
                println!("Worker 已缓存 Report: Operation={} Attempt={} Report={}", evidence.operation_id, evidence.attempt_id, evidence.report_id);
                deliveries.retain(&report, |delivery| hello::encode_report(delivery, &protocol))?;
            }
            _ = tick.tick() => {
                if draining && worker.pending() == 0 && deliveries.unconfirmed() == 0 && sends.is_empty() {
                    println!("Worker Draining 完成: 实例={instance}; Report 均已获得 Authority 收录回执, 发送尝试已结束; 不保证跨重启保留");
                    return Ok(());
                }
            }
            received = receipts.recv_message() => {
                match received {
                    Ok(message) => {
                        let result = (|| {
                            let receipt = ReportRetained::from_json(&message.payload)?;
                            ensure!(message.key_expr == receipt_inbox && message.key_expr == receipt.topic(), "Report 收录回执实际路由不符");
                            if deliveries.acknowledge(receipt.binding())? {
                                println!("Worker 已确认 Authority 收录 Report: Operation={} Attempt={} Report={}; 不表示已采用或持久化", receipt.operation_id, receipt.attempt_id, receipt.report_id);
                            }
                            Ok::<_, anyhow::Error>(())
                        })();
                        if let Err(error) = result { eprintln!("已忽略 Report 收录回执: {error}"); }
                    }
                    Err(error @ (beamlet_message_zenoh::Error::UnexpectedDelete | beamlet_message_zenoh::Error::MessageTooLarge)) => eprintln!("已忽略 Report 收录回执: {error}"),
                    Err(error) => return Err(error).context("接收 Report 收录回执失败"),
                }
            }
            received = transport.recv_message() => {
                let message = match received {
                    Ok(message) => message,
                    Err(error @ (beamlet_message_zenoh::Error::UnexpectedDelete | beamlet_message_zenoh::Error::MessageTooLarge)) => {
                        eprintln!("已忽略非法授权消息: {error}");
                        continue;
                    }
                    Err(error) => return Err(error).context("Worker 接收授权失败"),
                };
                if message.key_expr != inbox {
                    eprintln!("已忽略错误路由的授权消息");
                    continue;
                }
                let dispatch = match Dispatch::from_json(&message.payload, &protocol) {
                    Ok(dispatch) => dispatch.delivery(),
                    Err(error) => { eprintln!("授权无效, 未接纳: {error}"); continue; }
                };
                match worker.retained_dispatch(&dispatch) {
                    Err(error) => { eprintln!("授权未接纳: {error}"); continue; }
                    Ok(None) if shutdown.is_draining() => {
                        eprintln!("NodeShutdown: Worker 处于 Draining; 新 Attempt 未接纳, 不生成 Report");
                        continue;
                    }
                    Ok(None) if !deliveries.has_capacity(worker.pending()) => {
                        eprintln!("Worker 执行或活动 Report 交付槽位已满; 本次新 Attempt 未入队");
                        continue;
                    }
                    _ => {}
                }
                let receipt = AttemptAccepted::for_dispatch(&dispatch)?;
                match worker.receive_dispatch(dispatch) {
                    Ok(report) => {
                        if let Some(report) = report { deliveries.retain(&report, |delivery| hello::encode_report(delivery, &protocol))?; }
                        if sends.len() < limits.queue_capacity {
                            queue_send(&mut sends, &transport, Outgoing {
                                destination: receipt.topic(), bytes: receipt.to_json()?, purpose: SendPurpose::AttemptReceipt,
                            });
                        } else { eprintln!("Attempt 已接纳, 回执发送槽位已满; 不回滚接纳"); }
                    }
                    Err(error) => eprintln!("授权未接纳: {error}"),
                }
            }
        }
    }
}

pub(crate) fn worker_inbox(instance: RoleInstanceId) -> String {
    format!(
        "beamlet/v0/worker/{}/inbox",
        Uuid::from_u128(instance.as_u128()).simple()
    )
}

pub(crate) fn report_inbox(instance: RoleInstanceId) -> String {
    format!(
        "{NODE_INBOX_PREFIX}reports-{}",
        Uuid::from_u128(instance.as_u128()).simple()
    )
}

fn response_for(
    delivery: &Delivery<Task, String, String>,
    operation: OperationId,
) -> Result<Response> {
    let Message::SubmitReport(report) = &delivery.message else {
        bail!("完成结果不包含 Report");
    };
    ensure!(
        report.operation_id == operation,
        "Report 属于其他 Operation"
    );
    println!(
        "Authority 已收录: Operation={} Attempt={} Report={}",
        report.operation_id, report.attempt_id, report.report_id
    );
    Ok(Response::report(
        report,
        delivery.source_instance_id,
        delivery.target_instance_id,
    ))
}

async fn execute_hello(attempt: DispatchAttempt<Task>) -> SubmitReport<String, String> {
    let output = hello().to_owned();
    println!(
        "Worker 已执行 hello: Operation={} Attempt={}",
        attempt.operation.operation_id, attempt.attempt_id
    );
    SubmitReport {
        report_id: Uuid::new_v4().as_u128().into(),
        operation_id: attempt.operation.operation_id,
        attempt_id: attempt.attempt_id,
        impact: ImpactStatus::None,
        result: ExecutionResult::Succeeded { output },
    }
}

fn hello() -> &'static str {
    "World"
}

#[cfg(test)]
mod input_tests {
    use super::*;
    use beamlet::lifecycle::{Completion, Group, StopPolicy};

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn invalid_messages_do_not_stop_worker_or_prevent_next_authorization() {
        let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("tcp/{}", socket.local_addr().unwrap());
        drop(socket);
        let authority = Transport::listen(
            &endpoint,
            &report_inbox(1.into()),
            Limits::new(128 * 1024, 16).unwrap(),
        )
        .await
        .unwrap();
        let worker = Transport::connect(&endpoint, &worker_inbox(2.into()), Limits::default())
            .await
            .unwrap();
        let receipts = worker.subscribe(&receipt::inbox(2.into())).await.unwrap();
        let mut group =
            Group::new(tokio::runtime::Handle::current(), Duration::from_secs(5)).unwrap();
        group
            .start((), StopPolicy::Drain, |control| {
                serve_worker(worker, receipts, 2.into(), 1.into(), control)
            })
            .unwrap();
        timeout(Duration::from_secs(10), async {
            let sender = authority.sender(&worker_inbox(2.into())).await.unwrap();
            sender.wait_for_receiver().await.unwrap();
            let raw = zenoh::open(
                zenoh::Config::from_json5(
                    &serde_json::json!({
                        "mode": "client", "connect": {"endpoints": [endpoint]},
                        "scouting": {"multicast": {"enabled": false}, "gossip": {"enabled": false}}
                    })
                    .to_string(),
                )
                .unwrap(),
            )
            .await
            .unwrap();
            let publisher = raw.declare_publisher(worker_inbox(2.into())).await.unwrap();
            let matches = publisher.matching_listener().await.unwrap();
            while !publisher.matching_status().await.unwrap().matching() {
                matches.recv_async().await.unwrap();
            }
            publisher.delete().await.unwrap();
            publisher.put(vec![0; 65537]).await.unwrap();
            publisher.put(b"invalid json".to_vec()).await.unwrap();
            let mut ledger =
                HelloAuthority::new(1.into(), vec![2.into()], beamlet::memory::Limits::default())
                    .unwrap();
            let accepted = ledger
                .submit_keyed(3.into(), Task::Hello, || (4.into(), 5.into()))
                .unwrap();
            publisher
                .put(
                    Dispatch::from_delivery(&accepted.dispatch)
                        .unwrap()
                        .to_json(&ProtocolLimits::default())
                        .unwrap(),
                )
                .await
                .unwrap();
            let report =
                hello::decode_report(&authority.recv().await.unwrap(), &ProtocolLimits::default())
                    .unwrap();
            ledger.receive_report(report.clone()).unwrap();
            let receipt = ReportRetained::for_report(&report).unwrap();
            authority
                .sender(&receipt.topic())
                .await
                .unwrap()
                .send(receipt.to_json().unwrap())
                .await
                .unwrap();
            assert!(ledger.operation(4.into()).unwrap().adopted_report.is_some());
        })
        .await
        .unwrap();
        let summary = group.drain().await;
        assert!(!summary.timed_out);
        assert_eq!(
            summary.exits[0].result.as_ref().unwrap(),
            &Completion::Drained
        );
    }
}

#[cfg(test)]
mod local_progress_tests {
    use super::*;
    use beamlet::{
        lifecycle::{Completion, Group, StopPolicy},
        memory::OperationPhase,
        remote::Client,
    };
    use beamlet_message_zenoh::ClientTransport;
    use beamlet_protocol_demo::submission::HelloClientCodec;
    use tokio::sync::Notify;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn authority_answers_queries_while_a_local_operation_is_waiting() {
        timeout(Duration::from_secs(10), async {
            let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let endpoint = format!("tcp/{}", socket.local_addr().unwrap());
            drop(socket);
            let server = Arc::new(
                Transport::listen(&endpoint, "beamlet/test/local/submit/*", Limits::default())
                    .await
                    .unwrap(),
            );
            let console = Uuid::new_v4().simple().to_string();
            let mut client_transport = Transport::connect(
                &endpoint,
                &format!("beamlet/v0/console/{console}/inbox"),
                Limits::default(),
            )
            .await
            .unwrap();
            let started = Arc::new(Notify::new());
            let release = Arc::new(Notify::new());
            let execute = {
                let started = Arc::clone(&started);
                let release = Arc::clone(&release);
                move |attempt| {
                    let started = Arc::clone(&started);
                    let release = Arc::clone(&release);
                    async move {
                        started.notify_one();
                        release.notified().await;
                        execute_hello(attempt).await
                    }
                }
            };
            let mut group =
                Group::new(tokio::runtime::Handle::current(), Duration::from_secs(2)).unwrap();
            group
                .start(vec![Uuid::from_u128(1)], StopPolicy::Drain, |shutdown| {
                    serve_authority_with_executor(
                        server,
                        None,
                        AuthoritySetup {
                            instance: 1.into(),
                            workers: vec![2.into()],
                            local_worker: Some(
                                Worker::new(
                                    1.into(),
                                    vec![2.into()],
                                    beamlet::memory::Limits::default(),
                                )
                                .unwrap(),
                            ),
                            request_prefix: "beamlet/test/local/submit/".to_owned(),
                        },
                        shutdown,
                        execute,
                    )
                })
                .unwrap();
            {
                let sender = client_transport
                    .sender(&format!("beamlet/test/local/submit/{console}"))
                    .await
                    .unwrap();
                sender.wait_for_receiver().await.unwrap();
                sender
                    .send(
                        Submission::hello(Uuid::from_u128(7))
                            .to_json(&ProtocolLimits::default())
                            .unwrap(),
                    )
                    .await
                    .unwrap();
            }
            started.notified().await;
            let channel = ClientTransport::bind(
                &mut client_transport,
                &format!("{}{console}", query::prefix(1.into())),
            )
            .await
            .unwrap();
            let mut client = Client::new(channel, HelloClientCodec::default(), Some(1.into()));
            let snapshot = client.query_submission(1.into(), 7.into()).await.unwrap();
            assert_eq!(snapshot.phase, OperationPhase::Dispatched);
            assert!(snapshot.adopted_report.is_none());
            release.notify_one();
            let stopped = group.drain().await;
            assert!(
                stopped
                    .exits
                    .iter()
                    .all(|exit| matches!(exit.result, Ok(Completion::Drained)))
            );
        })
        .await
        .expect("local execution must not block Authority requests or drain");
    }
}
