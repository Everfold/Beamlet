use std::fmt;

/// 报告关联不匹配的原因, T 为拒绝后可取回的 Operation.
#[derive(Debug)]
#[non_exhaustive]
pub enum AcceptReportError<T> {
    /// 报告所属 Operation 与当前 Operation 不同.
    OperationMismatch {
        /// 未改变的原 Operation.
        operation: T,
    },
    /// 报告所属 Attempt 与当前等待的 Attempt 不同.
    AttemptMismatch {
        /// 未改变的原 Operation.
        operation: T,
    },
}

impl<T> AcceptReportError<T> {
    /// 取回原 Operation, 继续等待正确报告.
    #[must_use]
    pub fn into_operation(self) -> T {
        match self {
            Self::OperationMismatch { operation } | Self::AttemptMismatch { operation } => {
                operation
            }
        }
    }
}

impl<T> fmt::Display for AcceptReportError<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::OperationMismatch { .. } => "report belongs to a different operation",
            Self::AttemptMismatch { .. } => "report belongs to a different attempt",
        })
    }
}

impl<T: fmt::Debug> std::error::Error for AcceptReportError<T> {}
