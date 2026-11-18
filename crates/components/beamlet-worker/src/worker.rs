use crate::{BuildError, Capabilities, Context, ExecutionError, Options, presence};
use beamlet_api::{
    Pool, Presence, PresenceChannel, PresenceState, RoleInstance, TransportError, WorkerBinding,
    WorkerInbox,
};
use beamlet_primitives::Report;
use std::{collections::BTreeSet, future::Future};
use tokio::sync::watch;
use tracing::{Instrument, info, info_span, warn};

/// Worker 循环无法继续, 调用方可以停止同组角色并汇报错误.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RunError {
    /// 接收, 停止或公告通道失败.
    #[error(transparent)]
    Transport(
        /// 传输层返回的具体错误, 保留原始 source.
        #[from]
        TransportError,
    ),
    /// 停止信号来源失败.
    #[error("Worker shutdown signal failed: {0}")]
    Shutdown(
        /// 停止通知 future 返回的原始错误.
        #[source]
        ExecutionError,
    ),
    /// 执行任务 panic 或被取消; 不作为普通业务拒绝忽略.
    #[error("Worker 执行任务异常退出: {0}")]
    Task(
        /// 异步执行子任务 panic 或取消的原因.
        #[source]
        ExecutionError,
    ),
    /// 当前会话的报告身份序列耗尽.
    #[error("报告身份已耗尽")]
    ReportIdExhausted,
    /// 公告序列耗尽.
    #[error("心跳序号已耗尽")]
    PresenceSequenceExhausted,
    /// 公告刷新超过期限.
    #[error("Worker announcement flush timed out")]
    AnnouncementTimeout,
    /// 内部状态通道或公告循环提前结束.
    #[error("Worker 心跳任务意外结束")]
    AnnouncementStopped,
}

/// 已验证并冻结的 Worker, 身份, 公告和执行许可由同一份描述生成.
///
/// 并发名额持续到报告发送结束; 停止后排空已接收调用.
///
/// # 泛型参数
///
/// - `P`: 业务输入类型, 默认 String.
/// - `R`: 业务结果类型, 默认 String.
#[derive(Debug)]
pub struct Worker<P = String, R = String> {
    /// Worker 调用许可的固定快照, 包含 pool, 角色会话和启用能力.
    binding: WorkerBinding,
    /// 可选的人类可读标签, 不参与身份比较或消息路由.
    label: Option<String>,
    /// 当前 Worker 独立的并发, 心跳与公告期限配置.
    options: Options,
    /// Worker 可以调用的能力处理器集合; build 后仅保留已启用项.
    capabilities: Capabilities<P, R>,
}
/// 在运行前组装身份, 启用集合和独立参数.
///
/// # 泛型参数
///
/// - `P`: 业务输入类型, 默认 String.
/// - `R`: 业务结果类型, 默认 String.
#[derive(Debug)]
pub struct Builder<P = String, R = String> {
    /// 固定角色 UUID 和本次运行会话 UUID 组成的身份.
    instance: RoleInstance,
    /// Worker 或调用所属的唯一 pool.
    pool: Pool,
    /// 可选的人类可读标签, 不参与身份比较或消息路由.
    label: Option<String>,
    /// 当前 Worker 独立的并发, 心跳与公告期限配置.
    options: Options,
    /// Worker 可以调用的能力处理器集合; build 后仅保留已启用项.
    capabilities: Capabilities<P, R>,
    /// 显式指定的启用集合; None 表示保留全部已注册能力, Some 在 build 时校验.
    enabled: Option<BTreeSet<String>>,
}
impl<P, R> Worker<P, R> {
    /// 创建携带指定身份和处理器集合的 Worker 构造器.
    ///
    /// # 参数
    ///
    /// - `instance`: 当前角色的固定 UUID 和本次运行会话 UUID; 不会在此重新生成.
    /// - `capabilities`: 应用已注册的能力处理器集合.
    ///
    /// # 返回值
    ///
    /// 尚未绑定传输或启动任务的 Builder.
    pub fn builder(instance: RoleInstance, capabilities: Capabilities<P, R>) -> Builder<P, R> {
        Builder {
            instance,
            pool: Pool::default(),
            label: None,
            options: Options::default(),
            capabilities,
            enabled: None,
        }
    }
    /// 复制当前 Worker 的 pool, 会话身份和启用能力绑定.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Worker<P, R>` 的共享引用.
    ///
    /// # 返回值
    ///
    /// WorkerBinding, 供消息后端建立调用与公告端点.
    pub fn binding(&self) -> WorkerBinding {
        self.binding.clone()
    }
}
impl<P, R> Builder<P, R> {
    /// 设置 Worker 构造器的所属 pool.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Builder<P, R>` 的值; 调用后转移其所有权(Copy 类型按值复制).
    /// - `pool`: 路由或查询所属的 pool, 按精确名称隔离.
    ///
    /// # 返回值
    ///
    /// 更新 pool 后的 Builder 所有权.
    pub fn pool(mut self, pool: Pool) -> Self {
        self.pool = pool;
        self
    }
    /// 设置 Worker 的人类可读标签.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Builder<P, R>` 的值; 调用后转移其所有权(Copy 类型按值复制).
    /// - `label`: 便于人类识别的标签, 不参与身份校验或路由.
    ///
    /// # 返回值
    ///
    /// 保存标签后的 Builder 所有权; 标签不参与路由身份判断.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
    /// 设置 Worker 并发, 心跳与公告期限.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Builder<P, R>` 的值; 调用后转移其所有权(Copy 类型按值复制).
    /// - `options`: 当前模块的运行参数; 期限使用 Duration, 容量/并发由对应字段约束.
    ///
    /// # 返回值
    ///
    /// 更新运行参数后的 Builder 所有权, 校验在 build 时完成.
    pub fn options(mut self, options: Options) -> Self {
        self.options = options;
        self
    }
    /// 设置需要启用的能力名称集合, 重复项去重.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Builder<P, R>` 的值; 调用后转移其所有权(Copy 类型按值复制).
    /// - `enabled`: 需要启用的能力名称; 按集合语义处理, 不隐式注册未知能力.
    ///
    /// # 返回值
    ///
    /// 更新启用集合后的 Builder 所有权; 未知能力在 build 时拒绝.
    pub fn enabled(mut self, enabled: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.enabled = Some(enabled.into_iter().map(Into::into).collect());
        self
    }
    /// 校验身份, 参数和启用集合并构造 Worker.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Builder<P, R>` 的值; 调用后转移其所有权(Copy 类型按值复制).
    ///
    /// # 返回值
    ///
    /// 成功返回 Worker; 任何构造约束失败返回 BuildError, 不绑定传输或启动运行.
    ///
    /// # Errors
    /// 身份, 期限, 并发选项或启用能力集合无效时返回错误.
    pub fn build(mut self) -> Result<Worker<P, R>, BuildError> {
        if self.instance.role_id.is_nil() || self.instance.session_id.is_nil() {
            return Err(BuildError::InvalidIdentity);
        }
        self.options.validate()?;
        let enabled = self
            .enabled
            .unwrap_or_else(|| self.capabilities.ids().map(str::to_owned).collect());
        self.capabilities.retain_enabled(&enabled)?;
        Ok(Worker {
            binding: WorkerBinding {
                pool: self.pool,
                instance: self.instance,
                capabilities: enabled,
            },
            label: self.label,
            options: self.options,
            capabilities: self.capabilities,
        })
    }
}
impl<P: Send + 'static, R: Send + 'static> Worker<P, R> {
    /// 并行驱动 Worker 执行与公告, 停止时排空已接收调用.
    ///
    /// 接收端和公告通道必须来自使用 binding() 的同一次绑定. 运行在调用方的 Tokio
    /// 执行器上. 丢弃此 future 会中止异步子任务, 不承诺撤销已开始的阻塞计算.
    /// 处理器错误与报告发送失败释放名额并记录日志; 不重试或生成替代报告.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Worker<P, R>` 的值; 调用后转移其所有权(Copy 类型按值复制).
    /// - `inbox`: Worker 调用接收端; 停止后仍需消费已接收的队列.
    /// - `channel`: Worker 的探测, 连接变化和完整公告通道.
    /// - `shutdown`: 停止通知或 future; 完成后停止新接收并按角色语义排空.
    ///
    /// # 返回值
    ///
    /// 正常结束并完成离线收尾返回 `Ok(())`; 输入, 停止信号, 任务或公告驱动失败返回 RunError.
    ///
    /// # Errors
    /// 传输失败, 停止信号失败, 执行任务 panic 或公告循环失败时返回错误.
    pub async fn run<E>(
        self,
        mut inbox: impl WorkerInbox<P, R>,
        channel: impl PresenceChannel,
        shutdown: impl Future<Output = Result<(), E>>,
    ) -> Result<(), RunError>
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        let (state, changes) = watch::channel(PresenceState::Ready);
        let snapshot = Presence {
            pool: self.binding.pool.clone(),
            instance: self.binding.instance,
            label: self.label.clone(),
            capabilities: self.binding.capabilities.clone(),
            sequence: 0,
            state: PresenceState::Ready,
        };
        let announcements = presence::announce(channel, snapshot, self.options, changes);
        tokio::pin!(announcements);
        let result = {
            let work = self.execute(&mut inbox, state.clone(), shutdown);
            tokio::pin!(work);
            tokio::select! {
                result = &mut announcements => { result?; return Err(RunError::AnnouncementStopped); }
                result = &mut work => result,
            }
        };
        drop(inbox);
        state.send_replace(PresenceState::Offline);
        let announced = announcements.await;
        result.and(announced)
    }
    /// 按并发策略接收调用, 运行处理器并发送 Report.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Worker<P, R>` 的共享引用.
    /// - `inbox`: Worker 调用接收端; 停止后仍需消费已接收的队列.
    /// - `state`: 公告驱动读取的状态发送端, 停止接收时发送 Draining.
    /// - `shutdown`: 停止通知或 future; 完成后停止新接收并按角色语义排空.
    ///
    /// # 返回值
    ///
    /// 已接收队列和任务排空返回 `Ok(())`; 接收, 停止, 身份耗尽或任务异常返回 RunError.
    async fn execute<E>(
        &self,
        inbox: &mut impl WorkerInbox<P, R>,
        state: watch::Sender<PresenceState>,
        shutdown: impl Future<Output = Result<(), E>>,
    ) -> Result<(), RunError>
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        let session_id = self.binding.instance.session_id;
        let max_concurrent_tasks = self.options.concurrency.display_limit();
        tokio::pin!(shutdown);
        let mut stopping = false;
        let mut input_closed = false;
        let mut next_id = 0_u64;
        let mut tasks = tokio::task::JoinSet::new();

        info!(
            event = "role.ready",
            max_concurrent_tasks, "Worker 已就绪; 并发上限 0 表示无限"
        );

        while !input_closed || !tasks.is_empty() {
            let incoming = tokio::select! {
                // 满载时也及时停止新接收; 优先回收任务避免无限模式积压已完成句柄.
                biased;
                result = &mut shutdown, if !stopping => {
                    result.map_err(|error| RunError::Shutdown(ExecutionError::new(error)))?;
                    stopping = true;
                    state.send_replace(PresenceState::Draining);
                    inbox.stop().await?;
                    info!(event = "role.stopping", "停止接收新调用, 等待执行中任务和队列排空");
                    continue;
                }
                completed = tasks.join_next(), if !tasks.is_empty() => {
                    completed.expect("nonempty Worker task set").map_err(|error| RunError::Task(ExecutionError::new(error)))?;
                    continue;
                }
                incoming = inbox.receive(), if !input_closed && self.options.concurrency.admits(tasks.len()) => incoming?,
            };
            let Some(incoming) = incoming else {
                input_closed = true;
                continue;
            };

            // 接收顺序分配身份, 与执行/回报完成顺序无关.
            next_id = next_id.checked_add(1).ok_or(RunError::ReportIdExhausted)?;
            let report_id = format!("{session_id}:report:{next_id}");
            if !self.binding.accepts(&incoming.call) {
                warn!(event = "attempt.rejected", "目标身份、pool 或能力不匹配");
                continue;
            }
            let context = Context::from_call(&incoming.call);
            let handler = self
                .capabilities
                .handlers
                .get(&incoming.call.capability)
                .expect("binding and handlers share enabled capabilities")
                .clone();
            let span = info_span!(
                "attempt",
                operation_id = %incoming.call.attempt.operation_id(),
                attempt_id = %incoming.call.attempt.id(),
                capability = %incoming.call.capability,
                pool = %incoming.call.pool,
                worker_role_id = %incoming.call.worker.role_id,
                worker_session_id = %incoming.call.worker.session_id,
            );

            tasks.spawn(async move {
            info!(event = "operation.received", "收到 Operation 请求数据");
            info!(event = "attempt.received", "收到 Attempt");
            let result = handler(incoming.call.payload, context).await;
            match result {
                Ok(result) => {
                    let report = Report::new(report_id, incoming.call.attempt, result);
                    info!(event = "report.created", operation_id = %report.operation_id(), attempt_id = %report.attempt_id(), report_id = %report.id(), "已创建 Report");
                    let span = info_span!("report", report_id = %report.id());
                    async {
                        info!(event = "report.send_started", "开始发送 Report");
                        match incoming.reply.send(report).await {
                            Ok(()) => info!(event = "report.sent", "Report 已提交至传输层"),
                            // 原 Authority 已离线不应让这个 Worker 停止处理其他调用.
                            Err(error) => warn!(event = "report.send_failed", error = %error, "发送 Report 失败"),
                        }
                    }.instrument(span).await;
                }
                Err(error) => warn!(event = "attempt.rejected", error = %error, "拒绝执行 Attempt"),
            }
        }.instrument(span));
        }

        info!(event = "worker.drained", "已接收调用处理完毕");
        Ok(())
    }
}
