use std::{error::Error, fmt};

/// 台账更新的拒绝原因, 拒绝不会改变已有 Operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum LedgerError {
    /// Operation 身份已注册.
    OperationExists,
    /// 找不到对应 Operation.
    OperationNotFound,
    /// 当前有待处理的 Attempt.
    AttemptPending,
    /// 当前没有等待报告.
    NoPendingAttempt,
    /// Operation 已关闭.
    OperationClosed,
    /// 此台账中已签发过此 Attempt 身份.
    AttemptIdReused,
    /// 报告关联不匹配.
    ReportMismatch,
}

impl fmt::Display for LedgerError {
    /// 向格式化目标写入 `LedgerError` 的显示文本。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `LedgerError` 的共享引用。
    /// - `f`：接收格式化文本的写入目标。
    ///
    /// # 返回值
    ///
    /// 写入成功返回 `Ok(())`；格式化目标拒绝写入时返回 `fmt::Error`。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::OperationExists => "operation already exists",
            Self::OperationNotFound => "operation not found",
            Self::AttemptPending => "attempt pending",
            Self::NoPendingAttempt => "no pending attempt",
            Self::OperationClosed => "operation closed",
            Self::AttemptIdReused => "attempt identity already used",
            Self::ReportMismatch => "report does not match the pending attempt",
        })
    }
}

impl Error for LedgerError {}

/// 注册失败时保留调用者的非 Clone 请求与身份.
///
/// # 泛型参数
///
/// - `O`：Operation 身份类型。
/// - `P`：原始请求类型。
#[derive(Debug)]
pub struct CreateOperationError<O, P> {
    /// Operation 的固定身份，恢复或重复创建失败时仍保留输入所有权。
    pub(crate) id: O,
    /// Operation 的原始请求，不随生命周期转换改变。
    pub(crate) request: P,
}

impl<O, P> CreateOperationError<O, P> {
    /// 从重复创建错误中取回被拒绝的输入。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `CreateOperationError<O, P>` 的值；调用后转移其所有权（Copy 类型按值复制）。
    ///
    /// # 返回值
    ///
    /// 二元组 `(Operation 身份, 原始请求)` 的所有权。
    pub fn into_parts(self) -> (O, P) {
        (self.id, self.request)
    }
}

impl<O, P> fmt::Display for CreateOperationError<O, P> {
    /// 向格式化目标写入 `CreateOperationError<O, P>` 的显示文本。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `CreateOperationError<O, P>` 的共享引用。
    /// - `f`：接收格式化文本的写入目标。
    ///
    /// # 返回值
    ///
    /// 写入成功返回 `Ok(())`；格式化目标拒绝写入时返回 `fmt::Error`。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        LedgerError::OperationExists.fmt(f)
    }
}

impl<O: fmt::Debug, P: fmt::Debug> Error for CreateOperationError<O, P> {}
