//! Operation 请求, 报告及生命周期记录.
//!
//! 记录绑定请求与状态, 报告保留 Attempt 的关联与结果.

mod operation;
mod operation_record;
mod report;

pub use operation::Operation;
pub use operation_record::OperationRecord;
pub use report::Report;
