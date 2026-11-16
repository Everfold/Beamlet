use crate::Attempt;
use std::{error::Error, fmt};

/// 恢复等待状态时的 Operation 关联错误, 保留全部输入.
#[derive(Debug)]
pub struct RestoreOperationError<O, P, A> {
    pub(crate) id: O,
    pub(crate) request: P,
    pub(crate) attempt: Attempt<O, A>,
}

impl<O, P, A> RestoreOperationError<O, P, A> {
    /// 取回 Operation 身份, 请求及错误关联的授权, 无需克隆.
    pub fn into_parts(self) -> (O, P, Attempt<O, A>) {
        (self.id, self.request, self.attempt)
    }
}

impl<O, P, A> fmt::Display for RestoreOperationError<O, P, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Attempt does not belong to the restored Operation")
    }
}

impl<O: fmt::Debug, P: fmt::Debug, A: fmt::Debug> Error for RestoreOperationError<O, P, A> {}
