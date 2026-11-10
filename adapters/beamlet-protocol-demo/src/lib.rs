//! 演示协议 Adapter: 严格 v0 JSON, Hello 与固定测试任务; 不属于 Agent 契约
//!
//! 当前应用原型显式依赖此 crate. 通用领域与执行接口位于 beamlet, 不依赖这些固定任务或编码.
pub mod attempt_receipt;
pub mod demo;
pub mod executor;
pub mod hello;
pub mod lifecycle;
pub mod memory;
pub mod protocol;
pub mod query;
pub mod receipt;
pub mod submission;
pub mod worker_lifecycle;
