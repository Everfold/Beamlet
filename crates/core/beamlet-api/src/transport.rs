//! 消息交互契约, 只描述收发与连接观测, 不决定角色调度或生命周期.
use crate::{
    AuthorityInfo, Call, Command, Pool, Presence, Reply, Responder, RoleInstance, Uuid,
    WorkerReport, WorkerResponse,
};
use std::{collections::BTreeSet, error::Error, fmt, future::Future, sync::Arc, time::Duration};

/// 当前连接的观测世代; 世代变化使旧租约失效.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Connection {
    /// 本地单调递增的观测世代.
    pub epoch: u64,
    /// 是否可以尝试通信.
    pub connected: bool,
}

/// 共享的只读连接视图, 不暴露消息后端的通知原语.
#[derive(Clone)]
pub struct ConnectionView(
    /// 线程安全的当前连接状态读取函数, 克隆视图时共享.
    Arc<dyn Fn() -> Connection + Send + Sync>,
);
impl ConnectionView {
    /// 注入读取当前连接状态的非阻塞函数.
    ///
    /// # 参数
    ///
    /// - `read`: 线程安全, 非阻塞的连接状态读取函数.
    ///
    /// # 返回值
    ///
    /// 可克隆, 按需读取连接状态的视图; 构造时不建立连接.
    pub fn new(read: impl Fn() -> Connection + Send + Sync + 'static) -> Self {
        Self(Arc::new(read))
    }
    /// 读取当前连接状态快照.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `ConnectionView` 的共享引用.
    ///
    /// # 返回值
    ///
    /// 读取函数此刻返回的连接世代与 connected 状态.
    pub fn current(&self) -> Connection {
        (self.0)()
    }
}
impl fmt::Debug for ConnectionView {
    /// 向格式化目标写入 `ConnectionView` 的调试信息.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `ConnectionView` 的共享引用.
    /// - `f`: 接收格式化文本的写入目标.
    ///
    /// # 返回值
    ///
    /// 写入成功返回 `Ok(())`; 格式化目标拒绝写入时返回 `fmt::Error`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ConnectionView")
            .field(&self.current())
            .finish()
    }
}

/// 传输入口或连接失败, 保留底层错误原因.
#[derive(Debug)]
pub struct TransportError(
    /// 被包装的底层错误对象, 通过 Error::source 保留原因链.
    Box<dyn Error + Send + Sync>,
);
impl TransportError {
    /// 包装底层失败原因并保留错误链.
    ///
    /// # 参数
    ///
    /// - `source`: 底层失败原因; 包装后仍可通过错误 source 链访问.
    ///
    /// # 返回值
    ///
    /// 持有原始错误的具体错误对象, 不修改或重试失败操作.
    pub fn new(source: impl Into<Box<dyn Error + Send + Sync>>) -> Self {
        Self(source.into())
    }
}
impl fmt::Display for TransportError {
    /// 向格式化目标写入 `TransportError` 的显示文本.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `TransportError` 的共享引用.
    /// - `f`: 接收格式化文本的写入目标.
    ///
    /// # 返回值
    ///
    /// 写入成功返回 `Ok(())`; 格式化目标拒绝写入时返回 `fmt::Error`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl Error for TransportError {
    /// 借用被包装的底层错误.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `TransportError` 的共享引用.
    ///
    /// # 返回值
    ///
    /// 始终为 `Some` 的错误引用, 可继续访问其 source 链.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.0.as_ref())
    }
}

/// 交付的可确认程度, 不表示 Operation 的状态.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum DeliveryError {
    /// 指定目标没有接收者.
    #[error("当前没有匹配能力的接收者")]
    NoReceiver,
    /// 有明确证据证明未交付.
    #[error("明确未交付: {0}")]
    NotDelivered(
        /// 有证据表明调用未交付时的具体原因.
        String,
    ),
    /// 无法确定执行结果, 不可自动重试.
    #[error("交付结果未知: {0}")]
    Unknown(
        /// 无法确认远端是否执行时的原因, 不可据此自动重试.
        String,
    ),
}

/// 与角色绑定的接收许可, 不包含 Worker 选择策略.
#[derive(Debug, Clone)]
pub struct WorkerBinding {
    /// 唯一 pool.
    pub pool: Pool,
    /// 唯一角色会话.
    pub instance: RoleInstance,
    /// 本次实际启用的能力.
    pub capabilities: BTreeSet<String>,
}
impl WorkerBinding {
    /// 校验 Worker 绑定的身份和启用能力集合.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `WorkerBinding` 的共享引用.
    ///
    /// # 返回值
    ///
    /// 合法时返回 `Ok(())`; 空 UUID, 空能力集合或非法能力标识时返回 TransportError.
    ///
    /// # Errors
    /// UUID 为空, 没有启用能力或能力标识无效时返回错误.
    pub fn validate(&self) -> Result<(), TransportError> {
        if self.instance.role_id.is_nil()
            || self.instance.session_id.is_nil()
            || self.capabilities.is_empty()
            || self
                .capabilities
                .iter()
                .any(|cap| crate::CapabilityId::validate(cap).is_err())
        {
            return Err(TransportError::new("invalid worker binding"));
        }
        Ok(())
    }
    /// 检查调用是否匹配绑定的 pool, Worker 会话和能力许可.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `WorkerBinding` 的共享引用.
    /// - `call`: 已指定 pool, Authority 和 Worker 会话的调用, 包含 Attempt, 能力及业务输入.
    ///
    /// # 返回值
    ///
    /// 所有目标及关联字段合法且能力已启用时返回 `true`; 否则返回 `false`.
    pub fn accepts<P>(&self, call: &Call<P>) -> bool {
        call.pool == self.pool
            && call.worker == self.instance
            && !call.authority.role_id.is_nil()
            && !call.authority.session_id.is_nil()
            && !call.attempt.operation_id().is_empty()
            && !call.attempt.id().is_empty()
            && self.capabilities.contains(&call.capability)
    }
}

/// 一次调用与其独立的回复入口.
///
/// # 泛型参数
///
/// - `P`: 业务输入类型, 默认 String.
/// - `R`: 业务结果类型, 默认 String.
#[derive(Debug)]
pub struct Incoming<P = String, R = String> {
    /// 已签发的调用.
    pub call: Call<P>,
    /// 一次性报告或执行失败回复.
    pub reply: Responder<WorkerResponse<R>>,
}

/// Worker 接收端; stop 后保留已经接收的队列供排空.
///
/// receive 可安全取消; None 表示永久结束. stop 关闭新输入, 保留已接收队列供排空.
///
/// # 泛型参数
///
/// - `P`: 业务输入类型, 默认 String.
/// - `R`: 业务结果类型, 默认 String.
pub trait WorkerInbox<P = String, R = String>: Send {
    /// 接收一个带一次性回复入口的 Worker 调用.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `WorkerInbox` 的可变引用.
    ///
    /// # 返回值
    ///
    /// 返回 future; 等待完成后, future 完成后为 `Ok(Some(Incoming))`; 入口结束为 `Ok(None)`, 通信失败为错误.
    fn receive(
        &mut self,
    ) -> impl Future<Output = Result<Option<Incoming<P, R>>, TransportError>> + Send;
    /// 停止接收新调用, 同时保留已接收队列供 Worker 排空.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `WorkerInbox` 的可变引用.
    ///
    /// # 返回值
    ///
    /// 返回 future; 等待完成后, future 完成后成功为 `Ok(())`, 关闭或传输失败为错误.
    fn stop(&mut self) -> impl Future<Output = Result<(), TransportError>> + Send;
}

/// 向已选定实例交付一次, 不选人, 不重试.
///
/// 校验报告来源和授权关联; 超时只结束本地等待.
///
/// # 泛型参数
///
/// - `P`: 业务输入类型, 默认 String.
/// - `R`: 业务结果类型, 默认 String.
pub trait Delivery<P = String, R = String>: Clone + Send + Sync + 'static {
    /// 向已指定的 Worker 实例交付一次调用并等待关联报告.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Delivery` 的共享引用.
    /// - `call`: 已指定 pool, Authority 和 Worker 会话的调用, 包含 Attempt, 能力及业务输入.
    /// - `deadline`: 等待本次回复的最长时长, 非绝对时间; 超时不取消远端执行.
    ///
    /// # 返回值
    ///
    /// 返回 future; 等待完成后, future 成功返回完整 WorkerReport; 无接收者, 未交付或结果未知分别返回 DeliveryError.
    fn deliver(
        &self,
        call: Call<P>,
        deadline: Duration,
    ) -> impl Future<Output = Result<WorkerReport<R>, DeliveryError>> + Send;
}

/// 一份发现回复, 与发现请求关联.
#[derive(Debug, Clone)]
pub struct DiscoveryReply {
    /// 请求关联身份.
    pub request_id: Uuid,
    /// 应答的 Authority.
    pub authority: AuthorityInfo,
}

/// 逐条读取发现回复, 时间窗口与去重由调用者控制.
///
/// receive 可安全取消; None 表示入口永久结束.
pub trait DiscoveryInbox: Send {
    /// 逐条接收一次发现查询的回复.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `DiscoveryInbox` 的可变引用.
    ///
    /// # 返回值
    ///
    /// 返回 future; 等待完成后, future 返回 `Ok(Some(reply))`; 入口结束为 `Ok(None)`, 接收失败为错误.
    fn receive(
        &mut self,
    ) -> impl Future<Output = Result<Option<DiscoveryReply>, TransportError>> + Send;
}

/// 调用客户端所需的两种交互, 不保存 UI 选择或自动重发.
///
/// 支持并发调用; 关联校验由 Client 完成, 取消等待不撤销远端任务.
///
/// # 泛型参数
///
/// - `P`: 业务输入类型, 默认 String.
/// - `R`: 业务结果类型, 默认 String.
pub trait ClientTransport<P = String, R = String>: Clone + Send + Sync {
    /// 多回复接收入口.
    type Discovery: DiscoveryInbox;
    /// 为给定 pools 创建发现入口并发出查询.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `ClientTransport` 的共享引用.
    /// - `pools`: 本次发现或登记关注的 pool 集合, 按精确名称匹配.
    /// - `request_id`: 请求与回复关联 UUID; 不作为 Operation 身份或幂等凭据.
    ///
    /// # 返回值
    ///
    /// 返回 future; 等待完成后, future 成功返回逐条接收发现回复的端点; 准备或发送失败返回 TransportError.
    fn discover(
        &self,
        pools: &BTreeSet<Pool>,
        request_id: Uuid,
    ) -> impl Future<Output = Result<Self::Discovery, TransportError>> + Send;
    /// 向唯一 Authority 会话发送命令并等待一份回复.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `ClientTransport` 的共享引用.
    /// - `command`: 本次 Authority 命令, 携带请求 UUID, 目标会话及查询/提交数据.
    /// - `deadline`: 等待本次回复的最长时长, 非绝对时间; 超时不取消远端执行.
    ///
    /// # 返回值
    ///
    /// 返回 future; 等待完成后, future 成功返回含请求与 Authority 关联信息的 Reply; 传输或等待失败返回错误, 关联校验由调用者完成.
    fn request(
        &self,
        command: Command<P>,
        deadline: Duration,
    ) -> impl Future<Output = Result<Reply<R>, TransportError>> + Send;
}

/// Authority 控制入口的单个已解码事件.
///
/// # 泛型参数
///
/// - `P`: 业务输入类型, 默认 String.
/// - `R`: 业务结果类型, 默认 String.
#[derive(Debug)]
pub enum AuthorityEvent<P = String, R = String> {
    /// 连接变化, 旧世代缓冲必须先丢弃.
    Connection(
        /// 连接状态变化后的观测世代和 connected 状态.
        Connection,
    ),
    /// 路由匹配的完整 Worker 公告.
    Presence {
        /// Worker 自报的完整能力和运行状态快照.
        presence: Presence,
        /// 收到该公告时的连接世代, 必须与当前连接视图一致.
        epoch: u64,
    },
    /// 一个 pool 的发现查询.
    Discovery {
        /// 本次查询或调用所属的 pool, 按精确名称隔离.
        pool: Pool,
        /// 本次请求与回复的关联 UUID, 不是幂等键或 Operation 身份.
        request_id: Uuid,
        /// 发送 Authority 描述与请求 UUID 的一次性发现回复入口.
        reply: Responder<DiscoveryReply>,
    },
    /// 指向该实例的请求, 保留路由 pool 供角色校验.
    Request {
        /// 消息实际到达的路由 pool, Authority 还需核对它与命令内部 pool 一致.
        pool: Pool,
        /// 目标 Authority 的查询或提交命令.
        command: Command<P>,
        /// 发送本次控制或业务结果的独立回复入口.
        reply: Responder<Reply<R>>,
    },
}

/// 独立于业务队列的控制入口, 不持有 Worker 登记表.
///
/// receive 可安全取消; 重连须丢弃旧世代缓冲, 登记与选人由 Authority 负责.
///
/// # 泛型参数
///
/// - `P`: 业务输入类型, 默认 String.
/// - `R`: 业务结果类型, 默认 String.
pub trait AuthorityInbox<P = String, R = String>: Send {
    /// 取得能够持续读取当前连接状态的视图.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `AuthorityInbox` 的共享引用.
    ///
    /// # 返回值
    ///
    /// 连接视图句柄; 后续查询反映连接当前世代和状态.
    fn connection(&self) -> ConnectionView;
    /// 接收 Authority 的连接, 公告, 发现或业务请求事件.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `AuthorityInbox` 的可变引用.
    ///
    /// # 返回值
    ///
    /// 返回 future; 等待完成后, future 返回 `Ok(Some(event))`; 入口关闭为 `Ok(None)`, 接收失败为错误.
    fn receive(
        &mut self,
    ) -> impl Future<Output = Result<Option<AuthorityEvent<P, R>>, TransportError>> + Send;
    /// 请求所绑定 pools 中的 Worker 重新发布完整快照.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `AuthorityInbox` 的共享引用.
    ///
    /// # 返回值
    ///
    /// 返回 future; 等待完成后, 发送/通知完成返回 `Ok(())`; 传输失败返回错误, 不保证 Worker 已回复.
    fn probe(&self) -> impl Future<Output = Result<(), TransportError>> + Send;
}

/// Worker 重新公告的触发原因.
#[derive(Debug, Clone, Copy)]
pub enum PresenceEvent {
    /// 有 Authority 请求完整快照.
    Probe,
    /// 连接世代变化.
    Connection(
        /// 连接状态变化后的观测世代和 connected 状态.
        Connection,
    ),
}

/// Worker 公告与探测通道, 心跳周期和状态由角色决定.
///
/// 角色负责心跳, 序号和状态; 通道只传递事件, 刷新不代表远端已登记.
pub trait PresenceChannel: Send + Sync {
    /// 读取公告通道的当前连接状态.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `PresenceChannel` 的共享引用.
    ///
    /// # 返回值
    ///
    /// 包含观测世代与是否已连接的 Connection 快照.
    fn connection(&self) -> Connection;
    /// 等待 Worker 探测请求或连接状态变化.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `PresenceChannel` 的可变引用.
    ///
    /// # 返回值
    ///
    /// 返回 future; 等待完成后, 成功返回 Probe 或 Connection 事件; 观察入口关闭或传输失败返回错误.
    fn receive(&mut self) -> impl Future<Output = Result<PresenceEvent, TransportError>> + Send;
    /// 向绑定的 pool 发布 Worker 完整快照.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `PresenceChannel` 的共享引用.
    /// - `presence`: Worker 的完整状态快照, 包括 pool, 角色会话, 能力, 序号和运行状态.
    ///
    /// # 返回值
    ///
    /// 返回 future; 等待完成后, future 发送完成返回 `Ok(())`; 失败返回 TransportError, 不改变快照序号.
    fn publish(
        &self,
        presence: &Presence,
    ) -> impl Future<Output = Result<(), TransportError>> + Send;
    /// 等待传输缓冲完成刷新.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `PresenceChannel` 的共享引用.
    ///
    /// # 返回值
    ///
    /// 返回 future; 等待完成后, 刷新完成返回 `Ok(())`; 失败返回 TransportError, 成功不代表远端已接受业务.
    fn flush(&self) -> impl Future<Output = Result<(), TransportError>> + Send;
}
