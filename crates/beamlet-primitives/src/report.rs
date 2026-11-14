use crate::Attempt;

/// 一次 Attempt 执行结束后的结果证据, 身份和结果类型由调用者定义.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Report<O, A, R, V> {
    id: R,
    attempt: Attempt<O, A>,
    result: V,
}

impl<O, A, R, V> Report<O, A, R, V> {
    /// 消费已结束的 Attempt, 保留引用身份并接收唯一报告身份与结果.
    #[must_use]
    pub fn new(id: R, attempt: Attempt<O, A>, result: V) -> Self {
        Self {
            id,
            attempt,
            result,
        }
    }

    /// 借用报告身份.
    pub fn id(&self) -> &R {
        &self.id
    }

    /// 借用所属 Operation 身份.
    pub fn operation_id(&self) -> &O {
        self.attempt.operation_id()
    }

    /// 借用已结束的 Attempt 身份.
    pub fn attempt_id(&self) -> &A {
        self.attempt.id()
    }

    /// 借用结果内容, 是否满足请求由调用者判断.
    pub fn result(&self) -> &V {
        &self.result
    }

    /// 消费报告并取回身份, 授权和结果, 供外层编码.
    pub fn into_parts(self) -> (R, Attempt<O, A>, V) {
        (self.id, self.attempt, self.result)
    }

    /// 消费报告并取回结果所有权.
    pub fn into_result(self) -> V {
        self.result
    }
}
