use std::{error::Error, fmt};

/// 在线流程拒绝原因, 拒绝不会改变已有 Operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RuntimeError {
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
    /// 本次运行已签发过此 Attempt 身份.
    AttemptIdReused,
    /// 报告关联不匹配.
    ReportMismatch,
}
impl fmt::Display for RuntimeError {
    /// 输出无需业务数据的错误描述.
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
impl Error for RuntimeError {}

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
    /// 重复身份是注册的唯一拒绝原因.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        RuntimeError::OperationExists.fmt(f)
    }
}
impl<O: fmt::Debug, P: fmt::Debug> Error for CreateOperationError<O, P> {}
