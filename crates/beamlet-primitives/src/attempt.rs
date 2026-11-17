/// 针对一个 Operation 的一次执行授权, 由外层保存.
///
/// # 泛型参数
///
/// - `O`：Operation 身份类型。
/// - `A`：Attempt 身份类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Attempt<O, A> {
    /// 本次执行授权的身份。
    id: A,
    /// 该授权所属的 Operation 身份。
    operation_id: O,
}

impl<O, A> Attempt<O, A> {
    /// 从已有字段重建一次执行授权的数据。
    ///
    /// 调用者负责确认字段来源; 该数据结构不证明授权真实性.
    ///
    /// # 参数
    ///
    /// - `operation_id`：该授权所属的 Operation 身份。
    /// - `id`：已签发 Attempt 的身份。
    ///
    /// # 返回值
    ///
    /// 包含给定 Operation/Attempt 身份的 Attempt；不代表新签发或真实性校验。
    #[must_use]
    pub fn from_parts(operation_id: O, id: A) -> Self {
        Self { id, operation_id }
    }

    /// 消费 Attempt 并取回其关联身份。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Attempt<O, A>` 的值；调用后转移其所有权（Copy 类型按值复制）。
    ///
    /// # 返回值
    ///
    /// 二元组 `(Operation 身份, Attempt 身份)` 的所有权。
    pub fn into_parts(self) -> (O, A) {
        (self.operation_id, self.id)
    }

    /// 借用 Attempt 的身份。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Attempt<O, A>` 的共享引用。
    ///
    /// # 返回值
    ///
    /// Attempt 身份的只读引用。
    pub fn id(&self) -> &A {
        &self.id
    }

    /// 借用授权或报告所属的 Operation 身份。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Attempt<O, A>` 的共享引用。
    ///
    /// # 返回值
    ///
    /// 所属 Operation 身份的只读引用。
    pub fn operation_id(&self) -> &O {
        &self.operation_id
    }
}
