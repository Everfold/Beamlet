//! 消息交互契约, 只描述收发与连接观测, 不决定角色调度或生命周期.
use crate::{
    AuthorityInfo, Call, Command, Pool, Presence, Reply, Responder, RoleInstance, Uuid,
    WorkerReport,
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
pub struct ConnectionView(Arc<dyn Fn() -> Connection + Send + Sync>);
impl ConnectionView {
    /// 注入非阻塞的当前连接读取函数.
    pub fn new(read: impl Fn() -> Connection + Send + Sync + 'static) -> Self {
        Self(Arc::new(read))
    }
    /// 获取当前观测快照.
    pub fn current(&self) -> Connection {
        (self.0)()
    }
}
impl fmt::Debug for ConnectionView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ConnectionView")
            .field(&self.current())
            .finish()
    }
}

/// 传输入口或连接失败, 保留底层错误原因.
#[derive(Debug)]
pub struct TransportError(Box<dyn Error + Send + Sync>);
impl TransportError {
    /// 包装适配器错误, 不推断远端是否执行.
    pub fn new(source: impl Into<Box<dyn Error + Send + Sync>>) -> Self {
        Self(source.into())
    }
}
impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl Error for TransportError {
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
    NotDelivered(String),
    /// 无法确定执行结果, 不可自动重试.
    #[error("交付结果未知: {0}")]
    Unknown(String),
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
    /// 验证角色身份和启用能力集合.
    ///
    /// # Errors
    /// UUID 为空、没有启用能力或能力标识无效时返回错误.
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
    /// 是否符合目标与执行许可, 不修改原授权.
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
#[derive(Debug)]
pub struct Incoming<P = String, R = String> {
    /// 已签发的调用.
    pub call: Call<P>,
    /// 一次性报告回复.
    pub reply: Responder<WorkerReport<R>>,
}

/// Worker 接收端; stop 后保留已经接收的队列供排空.
pub trait WorkerInbox<P = String, R = String>: Send {
    /// 接收下一个调用; 等待可取消且不能丢弃已经取出的有效调用.
    fn receive(
        &mut self,
    ) -> impl Future<Output = Result<Option<Incoming<P, R>>, TransportError>> + Send;
    /// 停止接受新调用, 不取消已交出的任务.
    fn stop(&mut self) -> impl Future<Output = Result<(), TransportError>> + Send;
}

/// 向已选定实例交付一次, 不选人、不重试.
pub trait Delivery<P = String, R = String>: Clone + Send + Sync + 'static {
    /// 等待相关报告直到期限; 超时不撤销已签发的授权.
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
pub trait DiscoveryInbox: Send {
    /// 取消等待不丢失未返回的回复; None 表示结束.
    fn receive(
        &mut self,
    ) -> impl Future<Output = Result<Option<DiscoveryReply>, TransportError>> + Send;
}

/// 调用客户端所需的两种交互, 不保存 UI 选择或自动重发.
pub trait ClientTransport<P = String, R = String>: Clone + Send + Sync {
    /// 多回复接收入口.
    type Discovery: DiscoveryInbox;
    /// 创建发现入口并向目标 pools 查询.
    fn discover(
        &self,
        pools: &BTreeSet<Pool>,
        request_id: Uuid,
    ) -> impl Future<Output = Result<Self::Discovery, TransportError>> + Send;
    /// 向唯一 Authority 请求; 调用方校验返回的请求与会话关联.
    fn request(
        &self,
        command: Command<P>,
        deadline: Duration,
    ) -> impl Future<Output = Result<Reply<R>, TransportError>> + Send;
}

/// Authority 控制入口的单个已解码事件.
#[derive(Debug)]
pub enum AuthorityEvent<P = String, R = String> {
    /// 连接变化, 旧世代缓冲必须先丢弃.
    Connection(Connection),
    /// 路由匹配的完整 Worker 公告.
    Presence { presence: Presence, epoch: u64 },
    /// 一个 pool 的发现查询.
    Discovery {
        pool: Pool,
        request_id: Uuid,
        reply: Responder<DiscoveryReply>,
    },
    /// 指向该实例的请求, 保留路由 pool 供角色校验.
    Request {
        pool: Pool,
        command: Command<P>,
        reply: Responder<Reply<R>>,
    },
}

/// 独立于业务队列的控制入口, 不持有 Worker 登记表.
pub trait AuthorityInbox<P = String, R = String>: Send {
    /// 当前共享连接状态.
    fn connection(&self) -> ConnectionView;
    /// 读取事件; 等待可取消.
    fn receive(
        &mut self,
    ) -> impl Future<Output = Result<Option<AuthorityEvent<P, R>>, TransportError>> + Send;
    /// 请求所在 pools 的 Worker 重新公告.
    fn probe(&self) -> impl Future<Output = Result<(), TransportError>> + Send;
}

/// Worker 重新公告的触发原因.
#[derive(Debug, Clone, Copy)]
pub enum PresenceEvent {
    /// 有 Authority 请求完整快照.
    Probe,
    /// 连接世代变化.
    Connection(Connection),
}

/// Worker 公告与探测通道, 心跳周期和状态由角色决定.
pub trait PresenceChannel: Send + Sync {
    /// 当前连接状态.
    fn connection(&self) -> Connection;
    /// 等待探测或重连; 等待可取消.
    fn receive(&mut self) -> impl Future<Output = Result<PresenceEvent, TransportError>> + Send;
    /// 发送完整快照, 不增加序号或改变状态.
    fn publish(
        &self,
        presence: &Presence,
    ) -> impl Future<Output = Result<(), TransportError>> + Send;
    /// 等待传输缓冲完成发送, 不代表远端接受.
    fn flush(&self) -> impl Future<Output = Result<(), TransportError>> + Send;
}
