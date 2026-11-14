/// 针对一个 Operation 的一次执行授权, 由外层保存.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Attempt<O, A> {
    id: A,
    operation_id: O,
}

impl<O, A> Attempt<O, A> {
    /// 从已签发授权的字段重建数据, 不表示新建了一次授权.
    ///
    /// 调用者负责确认字段来源; 该数据结构不证明授权真实性.
    #[must_use]
    pub fn from_parts(operation_id: O, id: A) -> Self {
        Self { id, operation_id }
    }

    /// 消费授权数据并取回所属 Operation 身份与 Attempt 身份.
    pub fn into_parts(self) -> (O, A) {
        (self.operation_id, self.id)
    }

    /// 返回 Attempt 身份.
    pub fn id(&self) -> &A {
        &self.id
    }

    /// 返回所属 Operation 的身份.
    pub fn operation_id(&self) -> &O {
        &self.operation_id
    }
}
