//! Authority 与 Worker 共享的 Attempt 和 Report 数据, 不包含生命周期或 IO.

/// 可跨边界传递的既有执行授权数据.
mod attempt;
/// 保留原授权身份并携带业务结果的完整报告.
mod report;

pub use attempt::Attempt;
pub use report::Report;
