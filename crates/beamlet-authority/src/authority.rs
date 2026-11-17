use crate::{BuildError, Options, control::Inputs, engine, registry::Registry};
use beamlet_api::{
    AuthorityInbox, AuthorityInfo, ConnectionView, Delivery, Outcome, Pool, Responder, Submission,
    TransportError, WorkerView,
};
use std::{future::Future, time::Instant};

/// 本地提交与一次性回复入口; 不创建领域身份.
#[derive(Debug)]
pub struct Request<P = String, R = String> {
    /// 业务输入、pool 和目标约束.
    pub submission: Submission<P>,
    /// 返回 Completed、Rejected 或 Unresolved.
    pub reply: Responder<Outcome<R>>,
}
/// 可选本地提交来源, 例如终端或嵌入应用; EOF 不停止远端控制入口.
pub trait LocalInbox<P = String, R = String>: Send {
    /// 接收一个请求, 等待必须可取消; None 表示永久结束.
    ///
    /// # Errors
    /// 输入失败时保留底层错误; 返回错误会结束本次 Authority 运行.
    fn receive(
        &mut self,
    ) -> impl Future<Output = Result<Option<Request<P, R>>, TransportError>> + Send;
    /// 停止并丢弃未交出的请求, 不隐式签发 Attempt.
    ///
    /// # Errors
    /// 关闭输入失败时返回错误.
    fn stop(&mut self) -> impl Future<Output = Result<(), TransportError>> + Send;
}
struct Empty;
impl<P: Send, R: Send> LocalInbox<P, R> for Empty {
    async fn receive(&mut self) -> Result<Option<Request<P, R>>, TransportError> {
        Ok(None)
    }
    async fn stop(&mut self) -> Result<(), TransportError> {
        Ok(())
    }
}

/// 当前 Authority 的只读 Worker 视图, 不跨 Authority 共享所有权.
#[derive(Clone)]
pub struct Directory(pub(crate) Registry);
impl std::fmt::Debug for Directory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Directory").finish_non_exhaustive()
    }
}
impl Directory {
    /// 查询 pool 中的记录, 包括 draining、offline 和过期记录; stale 即时计算.
    pub fn workers(&self, pool: &Pool, capability: Option<&str>) -> Vec<WorkerView> {
        self.0.workers(pool, capability, Instant::now())
    }
}
/// 已验证的 Authority; 每次运行拥有独立台账和 Worker 登记表.
#[derive(Debug)]
pub struct Authority {
    info: AuthorityInfo,
    options: Options,
    directory: Directory,
}
impl Authority {
    /// 注入身份、运行参数和本次传输的连接视图, 不创建或保存 UUID.
    ///
    /// # Errors
    /// 身份、pools、容量或期限不合法时返回错误.
    pub fn new(
        info: AuthorityInfo,
        options: Options,
        connection: ConnectionView,
    ) -> Result<Self, BuildError> {
        if info.instance.role_id.is_nil() || info.instance.session_id.is_nil() {
            return Err(BuildError::InvalidIdentity);
        }
        if info.pools.is_empty() {
            return Err(BuildError::EmptyPools);
        }
        options.validate()?;
        let directory = Directory(Registry::new(
            info.pools.clone(),
            options.worker_timeout,
            connection,
        ));
        Ok(Self {
            info,
            options,
            directory,
        })
    }
    /// 获取可在运行期间查询的独立视图句柄.
    pub fn directory(&self) -> Directory {
        self.directory.clone()
    }
    /// 只接收远端控制与提交, 由外部停止信号结束并排空已签发工作.
    ///
    /// 端点匹配、缓冲刷新和取消约定与 [`Self::run_with_local`] 相同.
    ///
    /// # Errors
    /// 传输、回复、停止信号、任务或台账内部一致性失败时返回错误.
    pub async fn run<P, R, E>(
        self,
        endpoint: impl AuthorityInbox<P, R> + 'static,
        delivery: impl Delivery<P, R>,
        shutdown: impl Future<Output = Result<(), E>>,
    ) -> Result<(), RunError>
    where
        P: Clone + Send + 'static,
        R: Send + 'static,
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        self.run_with_local(endpoint, delivery, Empty, shutdown)
            .await
    }
    /// 同时接收远端与本地输入, 共用一次选择、签发与报告接受路径.
    ///
    /// endpoint 必须属于构造时的 AuthorityInfo 和连接视图. 调用方负责运行后刷新
    /// 共享传输的发送缓冲. 丢弃 future 会取消本地异步任务, 不撤销远端 Attempt.
    ///
    /// # Errors
    /// 输入、控制、交付子任务、回复、停止信号或台账内部一致性失败时返回错误.
    pub async fn run_with_local<P, R, E>(
        self,
        endpoint: impl AuthorityInbox<P, R> + 'static,
        delivery: impl Delivery<P, R>,
        local: impl LocalInbox<P, R>,
        shutdown: impl Future<Output = Result<(), E>>,
    ) -> Result<(), RunError>
    where
        P: Clone + Send + 'static,
        R: Send + 'static,
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        let input = Inputs::connect(
            endpoint,
            self.info.clone(),
            self.directory.0.clone(),
            local,
            self.options.input_capacity,
        )
        .await?;
        let dispatch = engine::Routed {
            transport: delivery,
            registry: self.directory.0,
            deadline: self.options.report_timeout,
        };
        engine::run(input, dispatch, self.info.instance, self.options, shutdown).await
    }
}

/// Authority 无法继续运行的原因, 不把未知交付伪装为 Report.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RunError {
    /// 控制或输入通信失败.
    #[error(transparent)]
    Transport(#[from] TransportError),
    /// 回复发送失败, 保留一次性发送错误.
    #[error(transparent)]
    Reply(#[from] beamlet_api::ReplyError),
    /// 调用方的停止信号失败.
    #[error("authority shutdown failed: {0}")]
    Shutdown(#[source] TransportError),
    /// 异步任务 panic 或取消.
    #[error("authority task failed: {0}")]
    Task(#[source] TransportError),
    /// 台账拒绝更新, 状态保持不变.
    #[error(transparent)]
    Ledger(#[from] beamlet_ledger::LedgerError),
    /// 内部序列或关联出现不可恢复错误.
    #[error("{0}")]
    Invariant(&'static str),
}
