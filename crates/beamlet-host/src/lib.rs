//! 一组长期任务的共同启动, 停止与有期限的排空, 使用调用方的 Tokio 执行器.
//!
//! Host 不认识角色类型, 身份格式, 传输, 配置文件或操作系统信号. 调用方装配任务,
//! 传入停止 future, 并负责传输刷新等资源收尾. 不自动重启任务.
//!
//! # Examples
//!
//! ```
//! use beamlet_host::{Host, Options};
//! use std::convert::Infallible;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut host = Host::new(Options::default())?;
//! let shutdown = host.shutdown();
//! host.add("service", shutdown.wait());
//! // 应用把 Ctrl+C, HTTP 管理接口或其他停止来源转换成 future.
//! host.run(async { Ok::<_, Infallible>(()) }).await?;
//! # Ok(())
//! # }
//! ```
/// 命名长期任务的统一启动, 停止, 排空与失败汇总.
mod host;
/// 本库运行参数, 默认值及构造时的语义校验.
mod options;
/// Host 发出的只读停止广播, 支持克隆与晚等待者.
mod shutdown;

pub use host::{Failure, Host, RunError};
pub use options::{BuildError, Options};
pub use shutdown::Shutdown;
