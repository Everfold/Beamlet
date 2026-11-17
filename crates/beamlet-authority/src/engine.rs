use crate::{Options, Request, RunError, registry::Registry};
use beamlet_api::{
    Call, DeliveryError, Outcome, Pool, Responder, RoleInstance, Selection, Submission,
    TransportError, WorkerReport,
};
use beamlet_ledger::Ledger;
use std::{
    collections::HashMap,
    future::Future,
    time::{Duration, Instant},
};
use tokio::task::JoinSet;
use tracing::{Instrument, info, info_span, warn};

pub(crate) type Reply<R = String> = Responder<Outcome<R>>;
pub(crate) trait Ingress<P = String, R = String>: Send {
    fn receive(&mut self) -> impl Future<Output = Result<Option<Request<P, R>>, RunError>> + Send;
    fn stop(&mut self) -> impl Future<Output = Result<(), RunError>> + Send;
}
pub(crate) trait Dispatch<P = String, R = String>: Clone + Send + Sync + 'static {
    fn select(
        &self,
        pool: &Pool,
        selection: Selection,
        capability: &str,
    ) -> Result<RoleInstance, DeliveryError>;
    fn deliver(
        &self,
        call: Call<P>,
    ) -> impl Future<Output = Result<WorkerReport<R>, DeliveryError>> + Send;
}
#[derive(Clone)]
pub(crate) struct Routed<T> {
    pub transport: T,
    pub registry: Registry,
    pub deadline: Duration,
}
impl<P: Send, R, T: beamlet_api::Delivery<P, R>> Dispatch<P, R> for Routed<T> {
    fn select(
        &self,
        pool: &Pool,
        selection: Selection,
        capability: &str,
    ) -> Result<RoleInstance, DeliveryError> {
        self.registry
            .select(pool, selection, capability, Instant::now())
            .ok_or(DeliveryError::NoReceiver)
    }
    async fn deliver(&self, call: Call<P>) -> Result<WorkerReport<R>, DeliveryError> {
        self.transport.deliver(call, self.deadline).await
    }
}
#[derive(Debug, thiserror::Error)]
enum FinishError {
    #[error(transparent)]
    Delivery(#[from] DeliveryError),
    #[error(transparent)]
    Ledger(#[from] beamlet_ledger::LedgerError),
    #[error("交付返回了不匹配或空身份的报告")]
    Mismatch,
}
/// 入口与交付分别注入; 每份交付独立等待, 不阻塞其他输入和报告.
pub(crate) async fn run<P, R, E>(
    mut ingress: impl Ingress<P, R>,
    delivery: impl Dispatch<P, R>,
    instance: RoleInstance,
    options: Options,
    shutdown: impl Future<Output = Result<(), E>>,
) -> Result<(), RunError>
where
    P: Clone + Send + 'static,
    R: Send + 'static,
    E: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    let max_concurrent_tasks = options.concurrency.display_limit();
    let max_concurrent_replies = options.max_concurrent_replies;
    let session_id = instance.session_id;
    let mut stopping = false;
    let mut ledger = Ledger::new();
    let mut replies: HashMap<String, (Reply<R>, RoleInstance, String)> = HashMap::new();
    let mut deliveries: JoinSet<(String, Result<WorkerReport<R>, DeliveryError>)> = JoinSet::new();
    let mut sends: JoinSet<Result<(), beamlet_api::ReplyError>> = JoinSet::new();
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
                result.map_err(|error| RunError::Shutdown(TransportError::new(error)))?;
                stopping = true;
                // 停止接收新请求, 只等待已经签发 Attempt 的调用.
                ingress.stop().await?;
                info!(event = "role.stopping", "停止接收新请求, 等待在途交付结束");
                None
            }

            sent = sends.join_next(), if !sends.is_empty() => {
                sent.expect("nonempty reply task set").map_err(|error| RunError::Task(TransportError::new(error)))??;
                None
            }

            completed = deliveries.join_next(), if !deliveries.is_empty() && sends.len() < max_concurrent_replies => {
                let (id, outcome) = completed.expect("nonempty delivery task set").map_err(|error| RunError::Task(TransportError::new(error)))?;
                let (reply, worker, attempt_id) = replies.remove(&id).ok_or(RunError::Invariant("缺少回复来源"))?;
                let report_id = outcome.as_ref().ok().map(|report| report.id().clone());
                let result = match finish(&mut ledger, &id, outcome) {
                    Ok(result) => Outcome::Completed { operation_id: id.clone(), attempt_id: attempt_id.clone(), report_id: report_id.ok_or(RunError::Invariant("缺少报告身份"))?, worker, result },
                    Err(error) => {
                        unresolved += 1;
                        warn!(event = "operation.unresolved", operation_id = %id, error = %error, "保持等待报告, 不重试");
                        Outcome::Unresolved { operation_id: id.clone(), attempt_id, worker, reason: error.to_string() }
                    }
                };

                sends.spawn(reply.send(result).in_current_span());
                None
            }

            request = ingress.receive(), if !stopping && sends.len() < max_concurrent_replies && options.concurrency.admits(deliveries.len()) => {
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
            next_id = next_id
                .checked_add(1)
                .ok_or(RunError::Invariant("操作身份已耗尽"))?;
            let id = format!("{session_id}:operation:{next_id}");

            ledger
                .create_operation(id.clone(), task.clone())
                .map_err(|_| RunError::Invariant("duplicate generated operation identity"))?;
            info!(event = "operation.created", operation_id = %id, "已创建 Operation");
            let attempt = ledger.create_attempt(&id, format!("{session_id}:attempt:{next_id}"))?;
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
                payload: task.payload,
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

    info!(
        event = "authority.drained",
        unresolved, "在途交付已结束; 内存状态不持久化"
    );

    Ok(())
}

/// 只有关联正确的 Report 能推进生命周期; 交付错误不伪装成报告.
fn finish<T, R>(
    ledger: &mut Ledger<String, String, T>,
    id: &str,
    outcome: Result<WorkerReport<R>, DeliveryError>,
) -> Result<R, FinishError> {
    let report = outcome?;
    let span = info_span!(
        "report",
        operation_id = %report.operation_id(),
        attempt_id = %report.attempt_id(),
        report_id = %report.id(),
    );
    let _entered = span.enter();
    info!(event = "report.received", "收到 Report");
    if report.operation_id() != id || report.id().is_empty() {
        return Err(FinishError::Mismatch);
    }

    ledger.accept_report(&report)?;
    info!(event = "report.accepted", "已接受 Report");
    ledger.close(report.operation_id())?;
    info!(event = "operation.closed", "已关闭 Operation");

    Ok(report.into_result())
}

#[cfg(test)]
#[path = "../tests/unit/engine.rs"]
mod tests;

#[cfg(test)]
#[path = "../tests/unit/responses.rs"]
mod response_tests;
