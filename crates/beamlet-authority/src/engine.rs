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

/// Authority 提交结果的一次性回复入口, 可返回完成, 拒绝或未解决状态.
///
/// # 泛型参数
///
/// - `R`: 业务结果类型, 默认 String.
pub(crate) type Reply<R = String> = Responder<Outcome<R>>;
/// Authority 编排循环的内部业务输入约定, 隔离控制处理与业务准入.
///
/// receive 可安全取消; stop 丢弃未签发请求, 已签发工作由编排循环排空.
///
/// # 泛型参数
///
/// - `P`: 业务输入类型, 默认 String.
/// - `R`: 业务结果类型, 默认 String.
pub(crate) trait Ingress<P = String, R = String>: Send {
    /// 向编排循环提供下一份结构化业务请求.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Ingress` 的可变引用.
    ///
    /// # 返回值
    ///
    /// 返回 future; 等待完成后, future 返回 `Ok(Some(Request))`; 入口结束返回 `Ok(None)`, 失败返回 RunError.
    fn receive(&mut self) -> impl Future<Output = Result<Option<Request<P, R>>, RunError>> + Send;
    /// 停止业务入口并丢弃未交出的请求.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Ingress` 的可变引用.
    ///
    /// # 返回值
    ///
    /// 返回 future; 等待完成后, future 停止成功返回 `Ok(())`, 输入收尾失败返回 RunError.
    fn stop(&mut self) -> impl Future<Output = Result<(), RunError>> + Send;
}
/// Authority 内部的选人与交付接口, 先选定一次实例, 再交付已签发调用.
///
/// 创建 Operation 前选择一次 Worker; Exact 失败不回退, 交付不重试.
///
/// # 泛型参数
///
/// - `P`: 业务输入类型, 默认 String.
/// - `R`: 业务结果类型, 默认 String.
pub(crate) trait Dispatch<P = String, R = String>: Clone + Send + Sync + 'static {
    /// 选择时固定的交付出口.
    type Route: Send + 'static;
    /// 按 pool, 能力和选择约束选定一个可用 Worker 会话.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Dispatch` 的共享引用.
    /// - `pool`: 查询, 候选选择或主题构造的目标 pool.
    /// - `selection`: Worker 选择约束: Any 选择可用候选, Exact 锁定角色与会话且不回退.
    /// - `capability`: 要调用或匹配的能力标识, 保留精确拼写.
    ///
    /// # 返回值
    ///
    /// 成功返回 `(RoleInstance, Route)`; 无匹配候选时返回 NoReceiver, Exact 不回退.
    fn select(
        &self,
        pool: &Pool,
        selection: Selection,
        capability: &str,
    ) -> Result<(RoleInstance, Self::Route), DeliveryError>;
    /// 交付已经签发且已确定 Worker 的调用.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Dispatch` 的共享引用.
    /// - `call`: 已指定 pool, Authority 和 Worker 会话的调用, 包含 Attempt, 能力及业务输入.
    /// - `route`: 选择时固定的出口, 不在交付失败后切换.
    ///
    /// # 返回值
    ///
    /// 返回 future; 等待完成后, 成功返回完整报告; 失败返回 DeliveryError, 不重新选择或重试.
    fn deliver(
        &self,
        call: Call<P>,
        route: Self::Route,
    ) -> impl Future<Output = Result<WorkerReport<R>, DeliveryError>> + Send;
}
/// 共用登记表, 按选定的出口交付一次.
pub(crate) struct Routed<P, R> {
    /// 注册顺序固定的交付出口.
    pub transports: crate::transports::Deliveries<P, R>,
    /// 此 Authority 的登记视图.
    pub registry: Registry,
    /// 单次报告期限.
    pub deadline: Duration,
}
impl<P, R> Clone for Routed<P, R> {
    /// 返回共享登记表和出口的句柄.
    fn clone(&self) -> Self {
        Self {
            transports: self.transports.clone(),
            registry: self.registry.clone(),
            deadline: self.deadline,
        }
    }
}
impl<P: Send + 'static, R: Send + 'static> Dispatch<P, R> for Routed<P, R> {
    /// 传输注册下标.
    type Route = usize;

    /// 按 pool, selection 和 capability 返回会话与出口; 无匹配则拒绝.
    fn select(
        &self,
        pool: &Pool,
        selection: Selection,
        capability: &str,
    ) -> Result<(RoleInstance, usize), DeliveryError> {
        self.registry
            .select(pool, selection, capability, Instant::now())
            .ok_or(DeliveryError::NoReceiver)
    }

    /// 通过选定的 route 交付 call, 返回报告; 失败不切换出口.
    async fn deliver(&self, call: Call<P>, route: usize) -> Result<WorkerReport<R>, DeliveryError> {
        self.transports[route].deliver(call, self.deadline).await
    }
}
/// 报告收尾失败的原因; 失败不被伪装成业务完成.
#[derive(Debug, thiserror::Error)]
enum FinishError {
    /// 指定交付未取得有效完整报告.
    #[error(transparent)]
    Delivery(
        /// 无接收者, 明确未交付或结果未知的交付失败分类.
        #[from]
        DeliveryError,
    ),
    /// 报告接受或关闭阶段被台账拒绝.
    #[error(transparent)]
    Ledger(
        /// 台账拒绝状态更新的错误.
        #[from]
        beamlet_ledger::LedgerError,
    ),
    /// 报告的 Operation 关联不符, 或报告身份为空.
    #[error("交付返回了不匹配或空身份的报告")]
    Mismatch,
}
/// 维护执行台账并并发等待交付和回复发送.
///
/// # 参数
///
/// - `ingress`: 提供业务请求和一次性回复入口的输入实现.
/// - `delivery`: 向已选定 Worker 交付 Call 并等待 Report 的实现.
/// - `instance`: 当前角色的固定 UUID 和本次运行会话 UUID; 不会在此重新生成.
/// - `options`: 当前模块的运行参数; 期限使用 Duration, 容量/并发由对应字段约束.
/// - `shutdown`: 停止通知或 future; 完成后停止新接收并按角色语义排空.
///
/// # 返回值
///
/// 入口结束或停止后排空完成返回 `Ok(())`; 输入, 回复, 任务或台账错误返回 RunError.
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
            let (worker, route) = match delivery.select(&pool, worker, &task.capability) {
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
                    let outcome = delivery.deliver(call, route).await;
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

/// 接受关联正确的报告并关闭对应 Operation.
///
/// # 参数
///
/// - `ledger`: 维护原始请求, 当前状态和已用授权身份的执行台账.
/// - `id`: 本次交付原先签发的 Operation 身份, 用于核对报告关联.
/// - `outcome`: 交付返回的完整报告或失败原因; 失败不会伪造 Report.
///
/// # 返回值
///
/// 成功返回报告业务结果的所有权; 交付, 关联或台账失败返回 FinishError.
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

/// 当前模块关键行为与边界条件的单元测试, 实现在 tests/unit 目录.
#[cfg(test)]
#[path = "../tests/unit/engine.rs"]
mod tests;

/// Authority 回复背压, 准入释放和停止排空的单元测试.
#[cfg(test)]
#[path = "../tests/unit/responses.rs"]
mod response_tests;
