//! 状态转换与关联校验规则.
//!
//! 外层定义身份类型并保存数据与状态, 内核校验输入并返回新状态.

mod operation;
mod report;

pub use operation::{AcceptReportError, OperationState};
pub use report::ReportAssociation;
