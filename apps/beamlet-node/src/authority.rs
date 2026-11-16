//! Authority 生命周期, 通过交付接口将任务发给一个匹配的 Worker.

use crate::{
    delivery::{Call, Delivery, DeliveryError, WorkerReport},
    ingress::{Ingress, Reply, Request},
    settings::Authority,
};
use anyhow::{Context, Result, ensure};
use beamlet_protocol::{Outcome, RoleInstance, Submission};
use beamlet_runtime::Runtime;
use std::{collections::HashMap, future::Future};
use tokio::task::JoinSet;
use tracing::{Instrument, info, info_span, warn};

#[cfg(test)]
mod response_tests;

/// 入口与交付分别注入; 每份交付独立等待, 不阻塞其他输入和报告.
pub(crate) async fn run(
    mut ingress: impl Ingress,
    delivery: impl Delivery,
    instance: RoleInstance,
    config: Authority,
    shutdown: impl Future<Output = std::io::Result<()>>,
) -> Result<()> {
    let Authority {
        max_concurrent_tasks,
        max_concurrent_replies,
        ..
    } = config;
    let session_id = instance.session_id;
    let mut stopping = false;
    let mut runtime = Runtime::new();
    let mut replies: HashMap<String, (Reply, RoleInstance, String)> = HashMap::new();
    let mut deliveries: JoinSet<(String, Result<WorkerReport, DeliveryError>)> = JoinSet::new();
    let mut sends: JoinSet<Result<()>> = JoinSet::new();
    let mut next_id = 0_u64;
    let mut unresolved = 0_usize;
    tokio::pin!(shutdown);

    info!(
        event = "role.ready",
        max_concurrent_tasks, max_concurrent_replies, "Authority 已就绪; 任务并发上限 0 表示无限"
    );

    loop {
        if stopping && deliveries.is_empty() && sends.is_empty() {
            break;
        }

        let incoming = tokio::select! {
            // 优先响应退出和回收已完成交付, 无限模式也不积压已完成句柄.
            biased;
            result = &mut shutdown, if !stopping => {
                result?;
                stopping = true;
                // 停止接收新请求, 只等待已经签发 Attempt 的调用.
                ingress.stop().await?;
                info!(event = "role.stopping", "停止接收新请求, 等待在途交付结束");
                None
            }

            sent = sends.join_next(), if !sends.is_empty() => {
                sent.context("缺少回复任务")?.context("回复任务异常退出")??;
                None
            }

            completed = deliveries.join_next(), if !deliveries.is_empty() && sends.len() < max_concurrent_replies => {
                let (id, outcome) = completed.context("缺少交付任务")?.context("交付任务异常退出")?;
                let (reply, worker, attempt_id) = replies.remove(&id).context("缺少回复来源")?;
                let report_id = outcome.as_ref().ok().map(|report| report.id().clone());
                let result = match finish(&mut runtime, &id, outcome) {
                    Ok(result) => Outcome::Completed { operation_id: id.clone(), attempt_id: attempt_id.clone(), report_id: report_id.context("缺少报告身份")?, worker, result },
                    Err(error) => {
                        unresolved += 1;
                        warn!(event = "operation.unresolved", operation_id = %id, error = %error, "保持等待报告, 不重试");
                        Outcome::Unresolved { operation_id: id.clone(), attempt_id, worker, reason: error.to_string() }
                    }
                };

                sends.spawn(reply.send(result).in_current_span());
                None
            }

            request = ingress.receive(), if !stopping && sends.len() < max_concurrent_replies && (max_concurrent_tasks == 0 || deliveries.len() < max_concurrent_tasks) => {
                let request = request?;
                if request.is_none() {
                    stopping = true;
                    ingress.stop().await?;
                }

                request
            }
        };

        if let Some(Request { submission, reply }) = incoming {
            let Submission { pool, worker, task } = submission;
            let worker = match delivery.select(&pool, worker, &task.capability) {
                Ok(worker) => worker,
                Err(error) => {
                    warn!(event = "request.rejected", %pool, capability = %task.capability, %error, "创建 Operation 前拒绝请求");
                    sends.spawn(
                        reply
                            .send(Outcome::Rejected {
                                reason: error.to_string(),
                            })
                            .in_current_span(),
                    );
                    continue;
                }
            };
            next_id = next_id.checked_add(1).context("操作身份已耗尽")?;
            let id = format!("{session_id}:operation:{next_id}");

            runtime.create_operation(id.clone(), task.clone())?;
            info!(event = "operation.created", operation_id = %id, "已创建 Operation");
            let attempt = runtime.create_attempt(&id, format!("{session_id}:attempt:{next_id}"))?;
            info!(
                event = "attempt.created",
                operation_id = %id,
                attempt_id = %attempt.id(),
                "已创建 Attempt"
            );
            let span = info_span!(
            "attempt",
            operation_id = %id,
            attempt_id = %attempt.id(),
                );

            replies.insert(id.clone(), (reply, worker, attempt.id().clone()));
            let call = Call {
                pool,
                authority: instance,
                worker,
                attempt,
                capability: task.capability,
                text: task.text,
            };

            let delivery = delivery.clone();
            deliveries.spawn(
                async move {
                    info!(event = "operation.send_started", "开始发送 Operation 请求数据");
                    info!(event = "attempt.send_started", "开始发送 Attempt");
                    let outcome = delivery.deliver(call).await;
                    if let Err(error) = &outcome {
                        warn!(event = "attempt.delivery_failed", error = %error, "未取得有效 Report");
                    }
                    (id, outcome)
                }
                .instrument(span),
            );
        }
    }

    ingress.flush().await?;
    info!(
        event = "authority.drained",
        unresolved, "在途交付已结束; 内存状态不持久化"
    );

    Ok(())
}

/// 只有关联正确的 Report 能推进生命周期; 交付错误不伪装成报告.
fn finish<T>(
    runtime: &mut Runtime<String, String, T>,
    id: &str,
    outcome: Result<WorkerReport, DeliveryError>,
) -> Result<String> {
    let report = outcome?;
    let span = info_span!(
        "report",
        operation_id = %report.operation_id(),
        attempt_id = %report.attempt_id(),
        report_id = %report.id(),
    );
    let _entered = span.enter();
    info!(event = "report.received", "收到 Report");
    ensure!(
        report.operation_id() == id,
        "交付返回了其他 Operation 的报告"
    );

    runtime.accept_report(&report)?;
    info!(event = "report.accepted", "已接受 Report");
    runtime.close(report.operation_id())?;
    info!(event = "operation.closed", "已关闭 Operation");

    Ok(report.into_result())
}

#[cfg(test)]
mod tests {
    use super::*;
    use beamlet_primitives::{Attempt, Report};

    #[test]
    fn errors_preserve_pending_attempt() {
        let id = "operation".to_owned();
        let mut runtime = Runtime::new();
        runtime
            .create_operation(id.clone(), String::from("hello"))
            .unwrap();
        let attempt = runtime.create_attempt(&id, "attempt".into()).unwrap();

        for error in [
            DeliveryError::NoReceiver,
            DeliveryError::NotDelivered("full".into()),
            DeliveryError::Unknown("timeout".into()),
        ] {
            assert!(finish(&mut runtime, &id, Err(error)).is_err());
            assert_eq!(
                runtime
                    .operation(&id)
                    .unwrap()
                    .pending_attempt_id()
                    .map(String::as_str),
                Some("attempt")
            );
        }

        for (operation, attempt) in [("other", "attempt"), ("operation", "stale")] {
            let report = Report::new(
                "bad".into(),
                Attempt::from_parts(operation.into(), attempt.into()),
                "bad".into(),
            );
            assert!(finish(&mut runtime, &id, Ok(report)).is_err());
            assert_eq!(
                runtime
                    .operation(&id)
                    .unwrap()
                    .pending_attempt_id()
                    .map(String::as_str),
                Some("attempt")
            );
        }

        let report = Report::new("good".into(), attempt, "olleh".into());
        assert_eq!(finish(&mut runtime, &id, Ok(report)).unwrap(), "olleh");
        assert!(runtime.create_attempt(&id, "new".into()).is_err());

        let duplicate = Report::new(
            "good".into(),
            Attempt::from_parts(id.clone(), "attempt".into()),
            "olleh".into(),
        );
        assert!(finish(&mut runtime, &id, Ok(duplicate)).is_err());
    }
}
