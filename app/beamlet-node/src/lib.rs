//! 固定 Hello 应用的角色与 Node 组装入口
//!
//! 应用显式组装 beamlet 驱动, 使用 beamlet-agent 的原型协议 Adapter
mod lifecycle;
pub mod node;
mod report_delivery;
pub mod roles;
mod shutdown;
