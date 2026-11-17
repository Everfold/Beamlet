//! 跨角色共享的身份、路由数据与一次性回复入口, 不绑定消息后端或配置来源.

mod call;
mod capability_id;
mod message;
mod pool;
mod responder;
mod roles;

pub use call::{Call, WorkerReport};
pub use capability_id::{CapabilityId, InvalidCapabilityId};
pub use message::{Command, Outcome, Reply, Submission, Task};
pub use pool::{InvalidPool, Pool};
pub use responder::{ReplyError, Responder};
pub use roles::{AuthorityInfo, Presence, PresenceState, RoleInstance, Selection, WorkerView};
pub use uuid::Uuid;
