//! 跨角色共享的身份, 路由数据与一次性回复入口, 不绑定消息后端或配置来源.

/// 带既有 Attempt 的调用数据与完整 WorkerReport 别名.
mod call;
/// 能力名称的字节长度校验及保持精确拼写的标识类型.
mod capability_id;
/// 查询, 提交及关联回复的语义数据.
mod message;
/// 可安全用于主题片段的 pool 名称及校验错误.
mod pool;
/// 可由任意适配器构造的一次性异步回复入口与发送错误.
mod responder;
/// 角色会话身份, Worker 状态快照与目标选择数据.
mod roles;
/// 传输中立的连接视图, 角色端点与交互 trait.
mod transport;
/// 各传输共享的能力标识和报告关联校验.
pub mod validation;

pub use call::{Call, WorkerReport};
pub use capability_id::{CapabilityId, InvalidCapabilityId};
pub use message::{Command, Outcome, Reply, Submission, Task};
pub use pool::{InvalidPool, Pool};
pub use responder::{ReplyError, Responder};
pub use roles::{AuthorityInfo, Presence, PresenceState, RoleInstance, Selection, WorkerView};
pub use uuid::Uuid;

pub use transport::{
    AuthorityEvent, AuthorityInbox, ClientTransport, Connection, ConnectionView, Delivery,
    DeliveryError, DiscoveryInbox, DiscoveryReply, Incoming, PresenceChannel, PresenceEvent,
    TransportError, WorkerBinding, WorkerInbox,
};
