use beamlet_kernel::{Attempt, ReportAssociation};

/// 一次 Attempt 结束后的结果证据, 报告身份与结果类型由调用者定义.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Report<O, A, R, V> {
    id: R,
    attempt: Attempt<O, A>,
    result: V,
}

impl<O, A, R, V> Report<O, A, R, V> {
    /// 消费已结束的 Attempt, 保留其关联并接收调用者生成的唯一报告身份.
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

    /// 借用所属 Operation 的身份.
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

    /// 克隆两个关联身份供内核校验, 不克隆报告身份或结果.
    #[must_use]
    pub fn association(&self) -> ReportAssociation<O, A>
    where
        O: Clone,
        A: Clone,
    {
        ReportAssociation {
            operation_id: self.operation_id().clone(),
            attempt_id: self.attempt_id().clone(),
        }
    }

    /// 消费报告并取回结果所有权.
    pub fn into_result(self) -> V {
        self.result
    }
}
