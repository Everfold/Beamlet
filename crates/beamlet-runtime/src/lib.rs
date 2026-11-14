//! 当前进程内的 Operation 管理与报告路由, 不负责交付, 调度或持久化.
//!
//! 所有变更通过 &mut self 串行执行. 关闭对象保留供查询与重复关闭.
//! 原语身份与请求类型由调用方定义, 请求不要求 Clone.

mod runtime;
mod runtime_error;

pub use runtime::{OperationView, Runtime};
pub use runtime_error::{CreateOperationError, RuntimeError};
