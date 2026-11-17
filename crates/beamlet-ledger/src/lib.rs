//! 执行台账, 保存 Operation 当前状态与已使用的 Attempt 身份.
//!
//! 所有变更通过 &mut self 串行执行. 关闭对象保留供查询与重复关闭.
//! 原语身份与请求类型由调用方定义, 请求不要求 Clone.
//! 台账不负责交付或调度, 不提供持久化或完整事件历史.

/// 多个 Operation 的在线执行台账及只读视图.
mod ledger;
/// 台账状态约束, 身份重复和报告关联失败的错误类型.
mod ledger_error;

pub use ledger::{Ledger, OperationView};
pub use ledger_error::{CreateOperationError, LedgerError};
