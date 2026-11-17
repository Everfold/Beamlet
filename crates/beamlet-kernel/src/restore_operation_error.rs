use crate::Attempt;
use std::{error::Error, fmt};

/// 恢复等待状态时的 Operation 关联错误, 保留全部输入.
///
/// # 泛型参数
///
/// - `O`：Operation 身份类型。
/// - `P`：原始请求类型。
/// - `A`：Attempt 身份类型。
#[derive(Debug)]
pub struct RestoreOperationError<O, P, A> {
    /// Operation 的固定身份，恢复或重复创建失败时仍保留输入所有权。
    pub(crate) id: O,
    /// Operation 的原始请求，不随生命周期转换改变。
    pub(crate) request: P,
    /// 恢复失败时归还的原始授权。
    pub(crate) attempt: Attempt<O, A>,
}

impl<O, P, A> RestoreOperationError<O, P, A> {
    /// 从恢复错误中取回身份、请求与授权。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `RestoreOperationError<O, P, A>` 的值；调用后转移其所有权（Copy 类型按值复制）。
    ///
    /// # 返回值
    ///
    /// 三元组 `(Operation 身份, 原始请求, Attempt)` 的所有权。
    pub fn into_parts(self) -> (O, P, Attempt<O, A>) {
        (self.id, self.request, self.attempt)
    }
}

impl<O, P, A> fmt::Display for RestoreOperationError<O, P, A> {
    /// 向格式化目标写入 `RestoreOperationError<O, P, A>` 的显示文本。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `RestoreOperationError<O, P, A>` 的共享引用。
    /// - `f`：接收格式化文本的写入目标。
    ///
    /// # 返回值
    ///
    /// 写入成功返回 `Ok(())`；格式化目标拒绝写入时返回 `fmt::Error`。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Attempt does not belong to the restored Operation")
    }
}

impl<O: fmt::Debug, P: fmt::Debug, A: fmt::Debug> Error for RestoreOperationError<O, P, A> {}
