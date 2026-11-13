/// 针对一个 Operation 的一次执行授权, 由外层保存.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Attempt<O, A> {
    pub(crate) id: A,
    pub(crate) operation_id: O,
}

impl<O, A> Attempt<O, A> {
    /// 返回 Attempt 身份.
    pub fn id(&self) -> &A {
        &self.id
    }

    /// 返回所属 Operation 的身份.
    pub fn operation_id(&self) -> &O {
        &self.operation_id
    }
}
