use crate::{BuildError, Options, control::Inputs, engine, registry::Registry};
use beamlet_api::{
    AuthorityInbox, AuthorityInfo, ConnectionView, Delivery, Outcome, Pool, Responder, Submission,
    TransportError, WorkerView,
};
use std::{future::Future, time::Instant};

/// 本地提交与一次性回复入口; 不创建领域身份.
///
/// # 泛型参数
///
/// - `P`: 业务输入类型, 默认 String.
/// - `R`: 业务结果类型, 默认 String.
#[derive(Debug)]
pub struct Request<P = String, R = String> {
    /// 业务输入, pool 和目标约束.
    pub submission: Submission<P>,
    /// 返回 Completed, Rejected 或 Unresolved.
    pub reply: Responder<Outcome<R>>,
}
/// 可选本地提交来源, 例如终端或嵌入应用; EOF 不停止远端控制入口.
///
/// receive 可安全取消; stop 丢弃未交出的提交, 不签发授权.
///
/// # 泛型参数
///
/// - `P`: 业务输入类型, 默认 String.
/// - `R`: 业务结果类型, 默认 String.
pub trait LocalInbox<P = String, R = String>: Send {
    /// 接收一个本地提交, EOF 不要求停止远端服务.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `LocalInbox` 的可变引用.
    ///
    /// # 返回值
    ///
    /// 返回 future; 等待完成后, future 返回 `Ok(Some(Request))`; 本地输入永久结束返回 `Ok(None)`, 输入失败返回错误.
    ///
    /// # Errors
    /// 输入失败时保留底层错误; 返回错误会结束本次 Authority 运行.
    fn receive(
        &mut self,
    ) -> impl Future<Output = Result<Option<Request<P, R>>, TransportError>> + Send;
    /// 停止本地输入并丢弃尚未交出的提交.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `LocalInbox` 的可变引用.
    ///
    /// # 返回值
    ///
    /// 返回 future; 等待完成后, future 关闭成功返回 `Ok(())`, 失败返回 TransportError; 不隐式签发 Attempt.
    ///
    /// # Errors
    /// 关闭输入失败时返回错误.
    fn stop(&mut self) -> impl Future<Output = Result<(), TransportError>> + Send;
}
/// 没有本地提交来源时使用的空输入, 立即返回 EOF 且停止操作幂等.
struct Empty;
impl<P: Send, R: Send> LocalInbox<P, R> for Empty {
    /// 表示没有配置本地输入来源.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Empty` 的可变引用.
    ///
    /// # 返回值
    ///
    /// 始终返回 `Ok(None)`.
    async fn receive(&mut self) -> Result<Option<Request<P, R>>, TransportError> {
        Ok(None)
    }
    /// 结束空本地输入, 保持幂等.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Empty` 的可变引用.
    ///
    /// # 返回值
    ///
    /// 始终返回 `Ok(())`.
    async fn stop(&mut self) -> Result<(), TransportError> {
        Ok(())
    }
}

/// 当前 Authority 的只读 Worker 视图, 不跨 Authority 共享所有权.
#[derive(Clone)]
pub struct Directory(
    /// 当前 Authority 的内部登记视图, 克隆时共享.
    pub(crate) Registry,
);
impl std::fmt::Debug for Directory {
    /// 向格式化目标写入 `Directory` 的调试信息.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Directory` 的共享引用.
    /// - `f`: 接收格式化文本的写入目标.
    ///
    /// # 返回值
    ///
    /// 写入成功返回 `Ok(())`; 格式化目标拒绝写入时返回 `fmt::Error`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Directory").finish_non_exhaustive()
    }
}
impl Directory {
    /// 查询 pool 中的 Worker 快照, 按可选能力过滤.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Directory` 的共享引用.
    /// - `pool`: 查询, 候选选择或主题构造的目标 pool.
    /// - `capability`: 可选能力过滤条件; None 返回该 pool 中所有能力的记录.
    ///
    /// # 返回值
    ///
    /// 匹配记录的 Vec, 包含 Ready, Draining, Offline 和过期状态; stale 在查询时计算.
    pub fn workers(&self, pool: &Pool, capability: Option<&str>) -> Vec<WorkerView> {
        self.0.workers(pool, capability, Instant::now())
    }
}
/// 已验证的 Authority; 每次运行拥有独立台账和 Worker 登记表.
#[derive(Debug)]
pub struct Authority {
    /// 当前 Authority 的角色会话, 标签和已加入 pools.
    info: AuthorityInfo,
    /// 已经通过构造校验的 Authority 运行参数.
    options: Options,
    /// 与当前 Authority 登记表共享状态的只读 Worker 查询句柄.
    directory: Directory,
}
impl Authority {
    /// 校验并构造独立的 Authority 编排实例.
    ///
    /// # 参数
    ///
    /// - `info`: Authority 的角色/会话身份, 标签及其加入的 pools.
    /// - `options`: 当前模块的运行参数; 期限使用 Duration, 容量/并发由对应字段约束.
    /// - `connection`: 非阻塞读取连接状态的视图, 连接世代变化使旧租约失效.
    ///
    /// # 返回值
    ///
    /// 成功返回拥有独立登记表的 Authority; 身份, pools 或运行参数非法返回 BuildError.
    ///
    /// # Errors
    /// 身份, pools, 容量或期限不合法时返回错误.
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
    /// 取得当前 Authority 登记表的只读查询句柄.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Authority` 的共享引用.
    ///
    /// # 返回值
    ///
    /// 可克隆 Directory, 与当前 Authority 共享登记视图.
    pub fn directory(&self) -> Directory {
        self.directory.clone()
    }
    /// 驱动远端控制与业务提交, 停止时排空已签发工作.
    ///
    /// 端点匹配, 缓冲刷新和取消约定与 [`Self::run_with_local`] 相同.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Authority` 的值; 调用后转移其所有权(Copy 类型按值复制).
    /// - `endpoint`: 属于当前 Authority 身份和 pools 的控制事件接收端.
    /// - `delivery`: 向已选定 Worker 交付 Call 并等待 Report 的实现.
    /// - `shutdown`: 停止通知或 future; 完成后停止新接收并按角色语义排空.
    ///
    /// # 返回值
    ///
    /// 正常停止返回 `Ok(())`; 传输, 回复, 停止信号, 任务或台账失败返回 RunError.
    ///
    /// # Errors
    /// 传输, 回复, 停止信号, 任务或台账内部一致性失败时返回错误.
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
    /// 合并本地与远端输入, 共用选择, 签发及报告接受流程.
    ///
    /// endpoint 必须属于构造时的 AuthorityInfo 和连接视图. 调用方负责运行后刷新
    /// 共享传输的发送缓冲. 丢弃 future 会取消本地异步任务, 不撤销远端 Attempt.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Authority` 的值; 调用后转移其所有权(Copy 类型按值复制).
    /// - `endpoint`: 属于当前 Authority 身份和 pools 的控制事件接收端.
    /// - `delivery`: 向已选定 Worker 交付 Call 并等待 Report 的实现.
    /// - `local`: 本地输入来源, 提供尚未签发授权的业务提交.
    /// - `shutdown`: 停止通知或 future; 完成后停止新接收并按角色语义排空.
    ///
    /// # 返回值
    ///
    /// 正常停止并排空已签发工作返回 `Ok(())`; 输入或编排失败返回 RunError.
    ///
    /// # Errors
    /// 输入, 控制, 交付子任务, 回复, 停止信号或台账内部一致性失败时返回错误.
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
    Transport(
        /// 传输层返回的具体错误, 保留原始 source.
        #[from]
        TransportError,
    ),
    /// 回复发送失败, 保留一次性发送错误.
    #[error(transparent)]
    Reply(
        /// 一次性结果回复发送失败的原因.
        #[from]
        beamlet_api::ReplyError,
    ),
    /// 调用方的停止信号失败.
    #[error("authority shutdown failed: {0}")]
    Shutdown(
        /// 停止 future 返回的错误, 以中立传输错误保留原始来源.
        #[source]
        TransportError,
    ),
    /// 异步任务 panic 或取消.
    #[error("authority task failed: {0}")]
    Task(
        /// 交付, 回复或控制子任务 panic/取消的原因.
        #[source]
        TransportError,
    ),
    /// 台账拒绝更新, 状态保持不变.
    #[error(transparent)]
    Ledger(
        /// 台账拒绝状态更新的错误.
        #[from]
        beamlet_ledger::LedgerError,
    ),
    /// 内部序列或关联出现不可恢复错误.
    #[error("{0}")]
    Invariant(
        /// 内部序列, 回复来源或关联不变量被破坏的诊断消息.
        &'static str,
    ),
}
