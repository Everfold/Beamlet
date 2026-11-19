use beamlet_api::{
    AuthorityEvent, AuthorityInbox, AuthorityInfo, ClientTransport, Command, Connection,
    ConnectionView, DeliveryError, DiscoveryInbox, Outcome, Pool, Presence, PresenceChannel,
    PresenceState, Reply, Responder, RoleInstance, Selection, Submission, Task, TransportError,
    Uuid, WorkerBinding, WorkerInbox,
};
use beamlet_authority::{Authority, Concurrency, Options};
use beamlet_primitives::Report;
use beamlet_transport_local::Local;
use std::{collections::BTreeSet, future::Future, time::Duration};
use tokio::{
    sync::{mpsc, oneshot, watch},
    time::timeout,
};
/// Authority 公开接口测试的等待期限: 3 秒, 回归时避免无限挂起.
const DEADLINE: Duration = Duration::from_secs(3);
/// 为测试生成独立的角色 UUID 和会话 UUID.
fn instance() -> RoleInstance {
    RoleInstance {
        role_id: Uuid::new_v4(),
        session_id: Uuid::new_v4(),
    }
}
/// 限制异步测试步骤的等待时长.
async fn within<T>(future: impl Future<Output = T>) -> T {
    timeout(DEADLINE, future)
        .await
        .expect("authority contract stalled")
}
/// 构造 Authority 测试用身份与 pool 描述.
fn info() -> AuthorityInfo {
    AuthorityInfo {
        instance: instance(),
        label: Some("authority".into()),
        pools: [Pool::default(), "other".parse().unwrap()].into(),
    }
}
/// 构造给指定 Authority 的测试文本提交命令.
fn submit(authority: RoleInstance, worker: Selection, text: &str) -> Command {
    Command::Submit {
        request_id: Uuid::new_v4(),
        authority,
        submission: Submission {
            pool: Pool::default(),
            worker,
            task: Task {
                capability: "echo/v1".into(),
                payload: text.into(),
            },
        },
    }
}
/// 通过真实本地传输查询 Authority 的 Worker 视图.
async fn workers(local: &Local, authority: RoleInstance) -> Vec<beamlet_api::WorkerView> {
    let reply = local
        .request(
            Command::Workers {
                request_id: Uuid::new_v4(),
                authority,
                pool: Pool::default(),
                capability: None,
            },
            DEADLINE,
        )
        .await
        .unwrap();
    let Outcome::Workers { workers } = reply.outcome else {
        panic!("workers expected");
    };
    workers
}
/// 验证 Authority 满载时控制处理仍推进, Exact 失效不回退.
#[tokio::test]
async fn control_progresses_at_capacity_and_exact_never_falls_back() {
    let scenario = async {
        let local = Local::new(16).unwrap();
        let info = info();
        let endpoint = local.bind_authority(&info).unwrap();
        let authority = Authority::new(
            info.clone(),
            Options {
                concurrency: Concurrency::Limited(1),
                input_capacity: 1,
                ..Options::default()
            },
        )
        .unwrap();
        let directory = authority.directory();
        let (stop, shutdown) = oneshot::channel();
        let running = tokio::spawn(authority.run((endpoint, local.clone()), shutdown));
        let target = instance();
        let binding = WorkerBinding {
            pool: Pool::default(),
            instance: target,
            capabilities: ["echo/v1".into()].into(),
        };
        let (mut worker, presence) = local.bind_worker(binding.clone()).unwrap();
        let mut snapshot = Presence {
            pool: Pool::default(),
            instance: target,
            label: Some("worker".into()),
            capabilities: binding.capabilities,
            sequence: 1,
            state: PresenceState::Ready,
        };
        presence.publish(&snapshot).await.unwrap();
        assert_eq!(workers(&local, info.instance).await.len(), 1);
        assert_eq!(
            directory.workers(&Pool::default(), None)[0]
                .presence
                .instance,
            target
        );
        // 第一份任务悬挂, 第二份排队, 第三份被队列拒绝; 控制查询不占业务槽位.
        let first = local.request(
            submit(info.instance, Selection::Exact(target), "first"),
            DEADLINE,
        );
        tokio::pin!(first);
        let incoming = tokio::select! {
            incoming = worker.receive() => incoming.unwrap().unwrap(),
            _ = &mut first => panic!("request finished before report"),
        };
        let second = local.request(submit(info.instance, Selection::Any, "queued"), DEADLINE);
        tokio::pin!(second);
        std::future::poll_fn(|cx| {
            assert!(second.as_mut().poll(cx).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        workers(&local, info.instance).await; // control has classified preceding submit
        let outcome = local
            .request(submit(info.instance, Selection::Any, "full"), DEADLINE)
            .await
            .unwrap()
            .outcome;
        assert!(matches!(outcome, Outcome::Rejected { .. }));
        let id = Uuid::new_v4();
        let mut discovered = local.discover(&info.pools, id).await.unwrap();
        assert_eq!(
            discovered
                .receive()
                .await
                .unwrap()
                .unwrap()
                .authority
                .instance,
            info.instance
        );
        snapshot.sequence += 1;
        snapshot.state = PresenceState::Offline;
        presence.publish(&snapshot).await.unwrap();
        assert_eq!(
            workers(&local, info.instance).await[0].presence.state,
            PresenceState::Offline
        );
        incoming
            .reply
            .send(Report::new(
                "report-1".into(),
                incoming.call.attempt,
                "done".into(),
            ))
            .await
            .unwrap();
        assert!(
            matches!(first.await.unwrap().outcome, Outcome::Completed { worker, result, .. } if worker == target && result == "done")
        );
        assert!(matches!(
            second.await.unwrap().outcome,
            Outcome::Rejected { .. }
        ));
        snapshot.sequence += 1;
        snapshot.state = PresenceState::Ready;
        presence.publish(&snapshot).await.unwrap();
        workers(&local, info.instance).await;
        let wrong_session = RoleInstance {
            session_id: Uuid::new_v4(),
            ..target
        };
        let outcome = local
            .request(
                submit(
                    info.instance,
                    Selection::Exact(wrong_session),
                    "old session",
                ),
                DEADLINE,
            )
            .await
            .unwrap()
            .outcome;
        assert!(matches!(outcome, Outcome::Rejected { .. }));
        // 关闭后明确无法交付仍返回 Unresolved, 不重新选择.
        worker.stop().await.unwrap();
        let outcome = local
            .request(
                submit(info.instance, Selection::Exact(target), "gone"),
                DEADLINE,
            )
            .await
            .unwrap()
            .outcome;
        assert!(
            matches!(outcome, Outcome::Unresolved { worker, reason, .. } if worker == target && reason == DeliveryError::NoReceiver.to_string())
        );
        stop.send(()).unwrap();
        running.await.unwrap().unwrap();
    };
    within(scenario).await;
}

/// 非文本求和请求, 用于验证 Authority 接收自定义业务类型.
#[derive(Clone)]
struct Numbers(
    /// 参与求和的整数列表; Clone 供台账与交付各自保存输入.
    Vec<u32>,
);
/// 自定义求和结果, 故意不实现 Clone, Debug 或序列化.
struct Sum(
    /// Worker 计算得到的整数和.
    u32,
);
/// 验证真实 Authority, Worker 与本地客户端使用自定义数据完成完整调用.
#[tokio::test]
async fn generic_authority_worker_and_client_use_public_interfaces_only() {
    let scenario = async {
        let local = Local::<Numbers, Sum>::new(8).unwrap();
        let info = info();
        let endpoint = local.bind_authority(&info).unwrap();
        let authority = Authority::new(info.clone(), Options::default()).unwrap();
        let mut capabilities = beamlet_worker::Capabilities::new();
        capabilities
            .register("sum/v1", |input: Numbers, _| async move {
                Ok(Sum(input.0.iter().sum()))
            })
            .unwrap();
        let worker_id = instance();
        let worker = beamlet_worker::Worker::builder(worker_id, capabilities)
            .build()
            .unwrap();
        let (inbox, presence) = local.bind_worker(worker.binding()).unwrap();
        let (stop_worker, worker_stop) = oneshot::channel();
        let worker = tokio::spawn(worker.run(inbox, presence, worker_stop));
        let (stop_authority, authority_stop) = oneshot::channel();
        let authority = tokio::spawn(authority.run((endpoint, local.clone()), authority_stop));
        loop {
            let reply = local
                .request(
                    Command::Workers {
                        request_id: Uuid::new_v4(),
                        authority: info.instance,
                        pool: Pool::default(),
                        capability: None,
                    },
                    DEADLINE,
                )
                .await
                .unwrap();
            if matches!(reply.outcome, Outcome::Workers { workers } if !workers.is_empty()) {
                break;
            }
            tokio::task::yield_now().await;
        }
        let reply = local
            .request(
                Command::Submit {
                    request_id: Uuid::new_v4(),
                    authority: info.instance,
                    submission: Submission {
                        pool: Pool::default(),
                        worker: Selection::Exact(worker_id),
                        task: Task {
                            capability: "sum/v1".into(),
                            payload: Numbers(vec![2, 3, 5]),
                        },
                    },
                },
                DEADLINE,
            )
            .await
            .unwrap();
        let Outcome::Completed {
            worker: executed,
            result,
            operation_id,
            attempt_id,
            report_id,
        } = reply.outcome
        else {
            panic!("completed expected");
        };
        assert_eq!(executed, worker_id);
        assert_eq!(result.0, 10);
        assert!(operation_id.starts_with(&info.instance.session_id.to_string()));
        assert!(attempt_id.starts_with(&info.instance.session_id.to_string()));
        assert!(report_id.starts_with(&worker_id.session_id.to_string()));
        stop_worker.send(()).unwrap();
        worker.await.unwrap().unwrap();
        stop_authority.send(()).unwrap();
        authority.await.unwrap().unwrap();
    };
    within(scenario).await;
}

/// 把测试有界通道适配为公开 LocalInbox 的输入.
struct LocalRequests(
    /// 待交给 Authority 的结构化本地请求队列.
    tokio::sync::mpsc::Receiver<beamlet_authority::Request>,
);
impl beamlet_authority::LocalInbox for LocalRequests {
    /// 从测试通道读取下一份结构化 Authority 提交.
    async fn receive(
        &mut self,
    ) -> Result<Option<beamlet_authority::Request>, beamlet_api::TransportError> {
        Ok(self.0.recv().await)
    }
    /// 停止测试输入, 丢弃尚未交出的业务请求.
    async fn stop(&mut self) -> Result<(), beamlet_api::TransportError> {
        self.0.close();
        while self.0.try_recv().is_ok() {}
        Ok(())
    }
}

/// 验证本地输入 EOF 后 Authority 仍接收远端请求.
#[tokio::test]
async fn local_eof_leaves_remote_requests_running() {
    let scenario = async {
        let local = Local::new(8).unwrap();
        let info = info();
        let endpoint = local.bind_authority(&info).unwrap();
        let authority = Authority::new(info.clone(), Options::default()).unwrap();
        let (send, receive) = tokio::sync::mpsc::channel(1);
        let (reply, result) = oneshot::channel();
        let Command::Submit { submission, .. } = submit(info.instance, Selection::Any, "local")
        else {
            unreachable!();
        };
        send.send(beamlet_authority::Request {
            submission,
            reply: beamlet_api::Responder::new(move |outcome| async move {
                reply
                    .send(outcome)
                    .map_err(|_| beamlet_api::ReplyError::new("caller closed"))
            }),
        })
        .await
        .unwrap();
        drop(send);
        let (stop, shutdown) = oneshot::channel();
        let running = tokio::spawn(authority.run_with_local(
            (endpoint, local.clone()),
            LocalRequests(receive),
            shutdown,
        ));
        assert!(matches!(result.await.unwrap(), Outcome::Rejected { .. }));
        let remote = local
            .request(submit(info.instance, Selection::Any, "remote"), DEADLINE)
            .await
            .unwrap();
        assert!(matches!(remote.outcome, Outcome::Rejected { .. }));
        stop.send(()).unwrap();
        running.await.unwrap().unwrap();
    };
    within(scenario).await;
}

/// 永不主动返回的交付替身, 用于观测开始执行和取消清理.
#[derive(Clone)]
struct PendingDelivery {
    /// deliver 已开始轮询的通知.
    started: std::sync::Arc<tokio::sync::Notify>,
    /// 交付 future 释放时触发的通知.
    dropped: std::sync::Arc<tokio::sync::Notify>,
}
/// 测试资源清理 guard; Drop 唤醒等待清理完成的观察者.
struct DropNotice(
    /// 资源被释放后发送通知的共享 Notify.
    std::sync::Arc<tokio::sync::Notify>,
);
impl Drop for DropNotice {
    /// 在测试资源被释放时发送一次 Drop 通知.
    fn drop(&mut self) {
        self.0.notify_one();
    }
}
impl beamlet_api::Delivery for PendingDelivery {
    /// 模拟一个已开始但永远不主动完成的交付.
    async fn deliver(
        &self,
        _call: beamlet_api::Call,
        _deadline: Duration,
    ) -> Result<beamlet_api::WorkerReport, DeliveryError> {
        let _notice = DropNotice(self.dropped.clone());
        self.started.notify_one();
        std::future::pending().await
    }
}

/// 验证取消 Authority 运行会释放交付等待和控制端点.
#[tokio::test]
async fn cancelling_authority_releases_delivery_and_control_endpoint() {
    let scenario = async {
        let local = Local::new(8).unwrap();
        let info = info();
        let endpoint = local.bind_authority(&info).unwrap();
        let authority = Authority::new(info.clone(), Options::default()).unwrap();
        let delivery = PendingDelivery {
            started: Default::default(),
            dropped: Default::default(),
        };
        let running = tokio::spawn(authority.run(
            (endpoint, delivery.clone()),
            std::future::pending::<std::io::Result<()>>(),
        ));
        let target = instance();
        let binding = WorkerBinding {
            pool: Pool::default(),
            instance: target,
            capabilities: ["echo/v1".into()].into(),
        };
        let (_inbox, presence) = local.bind_worker(binding.clone()).unwrap();
        presence
            .publish(&Presence {
                pool: binding.pool,
                instance: target,
                label: None,
                capabilities: binding.capabilities,
                sequence: 1,
                state: PresenceState::Ready,
            })
            .await
            .unwrap();
        assert_eq!(workers(&local, info.instance).await.len(), 1);
        let request = local.request(
            submit(info.instance, Selection::Exact(target), "pending"),
            DEADLINE,
        );
        tokio::pin!(request);
        tokio::select! {
            () = delivery.started.notified() => {},
            _ = &mut request => panic!("pending delivery completed"),
        }
        running.abort();
        assert!(running.await.unwrap_err().is_cancelled());
        delivery.dropped.notified().await;
        assert!(
            request.await.is_err(),
            "cancelled authority retained the reply"
        );
        loop {
            if local.bind_authority(&info).is_ok() {
                break;
            }
            tokio::task::yield_now().await;
        }
    };
    within(scenario).await;
}

/// 验证客户端提前丢弃发现入口或请求超时后，Authority 仍继续处理后续查询与提交。
#[tokio::test]
async fn abandoned_discovery_and_request_replies_keep_authority_running() {
    let scenario = async {
        let local = Local::new(8).unwrap();
        let info = info();
        let endpoint = local.bind_authority(&info).unwrap();
        let authority = Authority::new(info.clone(), Options::default()).unwrap();
        let (stop, shutdown) = oneshot::channel();
        let running = tokio::spawn(authority.run((endpoint, local.clone()), shutdown));

        let target = instance();
        let binding = WorkerBinding {
            pool: Pool::default(),
            instance: target,
            capabilities: ["echo/v1".into()].into(),
        };
        let (mut worker, presence) = local.bind_worker(binding.clone()).unwrap();
        presence
            .publish(&Presence {
                pool: Pool::default(),
                instance: target,
                label: None,
                capabilities: binding.capabilities,
                sequence: 1,
                state: PresenceState::Ready,
            })
            .await
            .unwrap();
        assert_eq!(workers(&local, info.instance).await.len(), 1);

        // 放弃发现接收端后发送发现请求，触发控制面 ReplyError。
        let abandoned_discovery = local.discover(&info.pools, Uuid::new_v4()).await.unwrap();
        drop(abandoned_discovery);

        // 请求在收到 Worker 报告前超时，触发业务面 ReplyError。
        let timed_out = local.request(
            submit(info.instance, Selection::Exact(target), "abandoned"),
            Duration::from_millis(20),
        );
        tokio::pin!(timed_out);
        let incoming = tokio::select! {
            incoming = worker.receive() => incoming.unwrap().unwrap(),
            _ = &mut timed_out => panic!("request finished before timeout"),
        };
        assert!(timed_out.await.is_err());
        incoming
            .reply
            .send(Report::new(
                "report-abandoned".into(),
                incoming.call.attempt,
                "ignored".into(),
            ))
            .await
            .unwrap();

        // 确认控制查询和后续正常提交均不受影响。
        assert_eq!(workers(&local, info.instance).await.len(), 1);
        let followup = local.request(
            submit(info.instance, Selection::Exact(target), "healthy"),
            DEADLINE,
        );
        tokio::pin!(followup);
        let incoming = tokio::select! {
            incoming = worker.receive() => incoming.unwrap().unwrap(),
            _ = &mut followup => panic!("followup finished before worker reply"),
        };
        incoming
            .reply
            .send(Report::new(
                "report-healthy".into(),
                incoming.call.attempt,
                "done".into(),
            ))
            .await
            .unwrap();
        assert!(
            matches!(followup.await.unwrap().outcome, Outcome::Completed { result, .. } if result == "done")
        );
        assert!(!running.is_finished());

        stop.send(()).unwrap();
        running.await.unwrap().unwrap();
    };
    within(scenario).await;
}

/// 可注入控制事件并固定返回 probe 错误的测试 AuthorityInbox。
struct StubAuthorityInbox {
    /// 待投递给控制循环的自定义控制事件。
    events: mpsc::Receiver<AuthorityEvent>,
    /// 只读连接状态视图。
    connection: ConnectionView,
    /// 每次调用 probe 时通知测试观察者。
    probed: mpsc::UnboundedSender<()>,
}

impl AuthorityInbox for StubAuthorityInbox {
    /// 返回连接视图。
    fn connection(&self) -> ConnectionView {
        self.connection.clone()
    }

    /// 接收下一条注入的控制事件。
    async fn receive(&mut self) -> Result<Option<AuthorityEvent>, TransportError> {
        Ok(self.events.recv().await)
    }

    /// 记录探测触发并故意返回传输错误，用于验证探测失败容错。
    async fn probe(&self) -> Result<(), TransportError> {
        let _ = self.probed.send(());
        Err(TransportError::new("probe unavailable"))
    }
}

/// 验证阻塞的控制回复不会阻断心跳观测与后续查询，且重连 probe 失败不终止 Authority。
#[tokio::test]
async fn blocked_control_reply_and_probe_failure_do_not_stall_or_stop_authority() {
    let scenario = async {
        let local = Local::new(8).unwrap();
        let info = info();
        let authority = Authority::new(info.clone(), Options::default()).unwrap();
        let directory = authority.directory();
        let (conn_tx, conn_rx) = watch::channel(Connection {
            epoch: 1,
            connected: true,
        });
        let (events_tx, events_rx) = mpsc::channel(8);
        let (probe_tx, mut probed) = mpsc::unbounded_channel();
        let inbox = StubAuthorityInbox {
            events: events_rx,
            connection: ConnectionView::new(move || *conn_rx.borrow()),
            probed: probe_tx,
        };
        let (stop, shutdown) = oneshot::channel();
        let running = tokio::spawn(authority.run((inbox, local), shutdown));

        // 启动时 probe 返回错误也不应终止 Authority。
        probed.recv().await.unwrap();
        assert!(!running.is_finished());

        // 注入一条会挂起的 Workers 控制回复。
        let (reply_started_tx, reply_started) = oneshot::channel();
        let (release_reply_tx, release_reply) = oneshot::channel();
        events_tx
            .send(AuthorityEvent::Request {
                pool: Pool::default(),
                command: Command::Workers {
                    request_id: Uuid::new_v4(),
                    authority: info.instance,
                    pool: Pool::default(),
                    capability: None,
                },
                reply: Responder::new(move |_| async move {
                    let _ = reply_started_tx.send(());
                    let _ = release_reply.await;
                    Ok(())
                }),
            })
            .await
            .unwrap();
        reply_started.await.unwrap();

        // 在上一条控制回复阻塞期间，心跳登记与新的控制查询仍可立即推进。
        let target = instance();
        events_tx
            .send(AuthorityEvent::Presence {
                presence: Presence {
                    pool: Pool::default(),
                    instance: target,
                    label: None,
                    capabilities: BTreeSet::from(["echo/v1".into()]),
                    sequence: 1,
                    state: PresenceState::Ready,
                },
                epoch: 1,
            })
            .await
            .unwrap();
        let (reply_tx, reply_rx) = oneshot::channel();
        events_tx
            .send(AuthorityEvent::Request {
                pool: Pool::default(),
                command: Command::Workers {
                    request_id: Uuid::new_v4(),
                    authority: info.instance,
                    pool: Pool::default(),
                    capability: None,
                },
                reply: Responder::new(move |reply| async move {
                    let _ = reply_tx.send(reply);
                    Ok(())
                }),
            })
            .await
            .unwrap();
        let Reply { outcome, .. } = reply_rx.await.unwrap();
        assert!(matches!(outcome, Outcome::Workers { workers } if workers.len() == 1));
        assert_eq!(directory.workers(&Pool::default(), None).len(), 1);

        // 连接重连后 probe 再次返回错误也不应停止 Authority。
        let reconnected = Connection {
            epoch: 2,
            connected: true,
        };
        conn_tx.send_replace(reconnected);
        events_tx
            .send(AuthorityEvent::Connection(reconnected))
            .await
            .unwrap();
        probed.recv().await.unwrap();
        assert!(!running.is_finished());

        release_reply_tx.send(()).unwrap();
        stop.send(()).unwrap();
        running.await.unwrap().unwrap();
    };
    within(scenario).await;
}
