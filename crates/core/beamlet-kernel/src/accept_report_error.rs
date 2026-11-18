use std::fmt;

/// 报告关联不匹配的原因, T 为拒绝后可取回的 Operation.
///
/// # 泛型参数
///
/// - `T`: 失败后可取回的 Operation 类型.
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
    /// 从报告校验错误中取回未被改变的 Operation.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `AcceptReportError<T>` 的值; 调用后转移其所有权(Copy 类型按值复制).
    ///
    /// # 返回值
    ///
    /// 原 Operation 的所有权, 供调用者继续等待正确报告.
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
    /// 向格式化目标写入 `AcceptReportError<T>` 的显示文本.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `AcceptReportError<T>` 的共享引用.
    /// - `formatter`: 接收格式化文本的写入目标.
    ///
    /// # 返回值
    ///
    /// 写入成功返回 `Ok(())`; 格式化目标拒绝写入时返回 `fmt::Error`.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::OperationMismatch { .. } => "report belongs to a different operation",
            Self::AttemptMismatch { .. } => "report belongs to a different attempt",
        })
    }
}

impl<T: fmt::Debug> std::error::Error for AcceptReportError<T> {}
