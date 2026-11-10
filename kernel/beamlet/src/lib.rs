//! 通用执行入口与内存账本, 由调用方显式组装角色并交付消息

mod authority;
mod worker;

pub use authority::Authority;
pub use beamlet_kernel::{id, model, state};
pub use worker::Worker;

pub mod executor;
pub mod memory;
