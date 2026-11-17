//! Operation, Attempt, Report 三种原语及其类型状态转换.
//!
//! Operation 按值推进生命周期, 身份与请求保持不变, 报告校验失败时归还原对象.
//!
//! 等待报告时不能再次创建 Attempt.
//! ```compile_fail
//! use beamlet_kernel::Operation;
//! let (operation, _) = Operation::new(1, "request").create_attempt(1);
//! let _ = operation.create_attempt(2);
//! ```
//!
//! 等待报告时不能关闭.
//! ```compile_fail
//! use beamlet_kernel::Operation;
//! let (operation, _) = Operation::new(1, "request").create_attempt(1);
//! let _ = operation.close();
//! ```
//!
//! 关闭后不能创建 Attempt.
//! ```compile_fail
//! use beamlet_kernel::Operation;
//! let operation = Operation::new(1, "request").close();
//! let _ = operation.create_attempt(1);
//! ```
//!
//! Ready 时不能重复接受已处理的报告.
//! ```compile_fail
//! use beamlet_kernel::{Operation, Report};
//! let (operation, attempt) = Operation::new(1, "request").create_attempt(1);
//! let report = Report::new(1, attempt, "result");
//! let operation = operation.accept_report(&report).unwrap();
//! let _ = operation.accept_report(&report);
//! ```
//!
//! 关闭后不能接受报告.
//! ```compile_fail
//! use beamlet_kernel::{Operation, Report};
//! let (operation, attempt) = Operation::new(1, "request").create_attempt(1);
//! let report = Report::new(1, attempt, "result");
//! let operation = operation.accept_report(&report).unwrap().close();
//! let _ = operation.accept_report(&report);
//! ```

/// 报告关联失败时保留原等待 Operation 的错误类型.
mod accept_report_error;
/// 内核合法状态转换图, 以及无条件转换结果的内部校验.
mod lifecycle;
/// Operation 的身份, 原始请求和 Ready/AwaitingReport/Closed 类型状态.
mod operation;
/// 恢复等待状态失败时归还全部输入的错误类型.
mod restore_operation_error;

pub use accept_report_error::AcceptReportError;
pub use beamlet_primitives::{Attempt, Report};
pub use operation::{AwaitingReport, Closed, Operation, OperationStatus, Ready};
pub use restore_operation_error::RestoreOperationError;
