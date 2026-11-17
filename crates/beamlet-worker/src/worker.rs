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
    /// 接收、停止或公告通道失败.
    #[error(transparent)]
    Transport(#[from] TransportError),
    /// 停止信号来源失败.
    #[error("Worker shutdown signal failed: {0}")]
    Shutdown(#[source] ExecutionError),
    /// 执行任务 panic 或被取消; 不作为普通业务拒绝忽略.
    #[error("Worker 执行任务异常退出: {0}")]
    Task(#[source] ExecutionError),
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

/// 已验证并冻结的 Worker, 身份、公告和执行许可由同一份描述生成.
#[derive(Debug)]
pub struct Worker<P = String, R = String> {
    binding: WorkerBinding,
    label: Option<String>,
    options: Options,
    capabilities: Capabilities<P, R>,
}
/// 在运行前组装身份、启用集合和独立参数.
#[derive(Debug)]
pub struct Builder<P = String, R = String> {
    instance: RoleInstance,
    pool: Pool,
    label: Option<String>,
    options: Options,
    capabilities: Capabilities<P, R>,
    enabled: Option<BTreeSet<String>>,
}
impl<P, R> Worker<P, R> {
    /// 显式注入身份和已注册能力; 不生成或保存 UUID.
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
    /// 取得传输绑定描述; 从 Worker 已启用的处理器生成.
    pub fn binding(&self) -> WorkerBinding {
        self.binding.clone()
    }
}
impl<P, R> Builder<P, R> {
    /// 选择唯一 pool, 默认为 default.
    pub fn pool(mut self, pool: Pool) -> Self {
        self.pool = pool;
        self
    }
    /// 设置可选的人类可读名称.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
    /// 替换运行参数; 在 build 中统一校验.
    pub fn options(mut self, options: Options) -> Self {
        self.options = options;
        self
    }
    /// 只启用给定能力, 重复项去重; 未调用时启用全部注册能力.
    pub fn enabled(mut self, enabled: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.enabled = Some(enabled.into_iter().map(Into::into).collect());
        self
    }
    /// 验证并冻结配置和执行许可.
    ///
    /// # Errors
    /// 身份、期限、并发选项或启用能力集合无效时返回错误.
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
    /// 驱动已绑定的调用与公告通道, 等待外部停止后排空队列和在途任务.
    ///
    /// 接收端和公告通道必须来自使用 binding() 的同一次绑定. 运行在调用方的 Tokio
    /// 执行器上. 丢弃此 future 会中止异步子任务, 不承诺撤销已开始的阻塞计算.
    /// 处理器错误与报告发送失败释放名额并记录日志; 不重试或生成替代报告.
    ///
    /// # Errors
    /// 传输失败、停止信号失败、执行任务 panic 或公告循环失败时返回错误.
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
