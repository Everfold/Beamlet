//! 跨角色共享的身份、路由数据与一次性回复入口, 不绑定消息后端或配置来源.

mod pool;
mod responder;
mod roles;

pub use pool::{InvalidPool, Pool};
pub use responder::{ReplyError, Responder};
pub use roles::{AuthorityInfo, Presence, PresenceState, RoleInstance, Selection, WorkerView};
pub use uuid::Uuid;
