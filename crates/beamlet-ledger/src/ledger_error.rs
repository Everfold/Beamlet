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
#[derive(Debug)]
pub struct CreateOperationError<O, P> {
    pub(crate) id: O,
    pub(crate) request: P,
}

impl<O, P> CreateOperationError<O, P> {
    /// 取回被拒绝的身份和原始请求.
    pub fn into_parts(self) -> (O, P) {
        (self.id, self.request)
    }
}

impl<O, P> fmt::Display for CreateOperationError<O, P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        LedgerError::OperationExists.fmt(f)
    }
}

impl<O: fmt::Debug, P: fmt::Debug> Error for CreateOperationError<O, P> {}
