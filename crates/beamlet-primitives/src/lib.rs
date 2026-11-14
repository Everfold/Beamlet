//! Authority 与 Worker 共享的 Attempt 和 Report 数据, 不包含生命周期或 IO.

mod attempt;
mod report;

pub use attempt::Attempt;
pub use report::Report;
