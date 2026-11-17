//! 可嵌入的 Worker: 注册异步能力、执行调用、创建报告并维护完整心跳.
//!
//! 使用调用方的 Tokio 执行器, 不创建运行时、读取配置或连接消息服务器.
//! [`Worker::run`] 消费 Worker 和已绑定的接收/公告端点; 停止后排空已接收调用.
//!
//! ```
//! use beamlet_api::{RoleInstance, Uuid};
//! use beamlet_worker::{Capabilities, Concurrency, Options, Worker};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut capabilities = Capabilities::new();
//! capabilities.register("echo/v1", |text: String, _context| async move { Ok(text) })?;
//! let worker = Worker::builder(
//!     RoleInstance { role_id: Uuid::new_v4(), session_id: Uuid::new_v4() },
//!     capabilities,
//! ).options(Options { concurrency: Concurrency::Limited(4), ..Options::default() })
//!  .enabled(["echo/v1"])
//!  .build()?;
//! let binding = worker.binding();
//! assert!(binding.capabilities.contains("echo/v1"));
//! # Ok(())
//! # }
//! ```
/// 能力注册、启用校验和业务处理器上下文。
mod capabilities;
/// 本库运行参数、默认值及构造时的语义校验。
mod options;
/// Worker 完整公告的周期、探测、重连与停止状态驱动。
mod presence;
/// Worker 构造、并发执行、报告创建及公告驱动的生命周期。
mod worker;

pub use capabilities::{Capabilities, CapabilityError, Context, ExecutionError};
pub use options::{BuildError, Concurrency, Options};
pub use worker::{Builder, RunError, Worker};
