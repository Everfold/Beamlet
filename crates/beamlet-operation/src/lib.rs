//! 不可变的 Operation 请求及其生命周期记录.
//!
//! 记录将请求与当前状态绑定, 调用内核后保存成功返回的新状态.

mod operation;
mod operation_record;

pub use operation::Operation;
pub use operation_record::OperationRecord;
