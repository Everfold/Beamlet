use crate::{Attempt, Report, lifecycle};

/// 没有等待报告的 Attempt, 可以创建授权或结束 Operation.
#[derive(Debug)]
pub struct Ready {
    machine: lifecycle::Lifecycle<(), lifecycle::Ready>,
}

/// 等待指定 Attempt 的报告, 只能接受报告后继续推进.
#[derive(Debug)]
pub struct AwaitingReport<A> {
    machine: lifecycle::Lifecycle<(), lifecycle::AwaitingReport>,
    attempt_id: A,
}

/// Operation 已结束, 允许重复关闭.
#[derive(Debug)]
pub struct Closed {
    machine: lifecycle::Lifecycle<(), lifecycle::Closed>,
}

/// 身份与请求保持不变的 Operation, S 表达当前生命周期状态.
#[derive(Debug)]
#[must_use]
pub struct Operation<O, P, S = Ready> {
    id: O,
    request: P,
    state: S,
}

impl<O, P, S> Operation<O, P, S> {
    /// 借用 Operation 身份.
    pub fn id(&self) -> &O {
        &self.id
    }

    /// 借用原始请求, 生命周期转换不会修改它.
    pub fn request(&self) -> &P {
        &self.request
    }
}

impl<O, P> Operation<O, P> {
    /// 接收调用者生成的唯一身份与请求, 从 Ready 开始.
    pub fn new(id: O, request: P) -> Self {
        Self {
            id,
            request,
            state: Ready {
                machine: lifecycle::Lifecycle::new(()),
            },
        }
    }

    /// 消费当前 Operation, 返回等待状态的 Operation 与新的执行授权.
    ///
    /// 调用者提供未使用的 Attempt 身份. 两个身份各克隆一次供授权独立持有.
    pub fn create_attempt<A: Clone>(
        self,
        attempt_id: A,
    ) -> (Operation<O, P, AwaitingReport<A>>, Attempt<O, A>)
    where
        O: Clone,
    {
        let attempt = Attempt {
            operation_id: self.id.clone(),
            id: attempt_id.clone(),
        };
        let operation = Operation {
            id: self.id,
            request: self.request,
            state: AwaitingReport {
                machine: lifecycle::transition(self.state.machine.create_attempt()),
                attempt_id,
            },
        };
        (operation, attempt)
    }

    /// 消费 Ready Operation, 返回 Closed Operation.
    pub fn close(self) -> Operation<O, P, Closed> {
        Operation {
            id: self.id,
            request: self.request,
            state: Closed {
                machine: lifecycle::transition(self.state.machine.close()),
            },
        }
    }
}

impl<O, P, A> Operation<O, P, AwaitingReport<A>> {
    /// 借用当前等待的 Attempt 身份.
    pub fn pending_attempt_id(&self) -> &A {
        &self.state.attempt_id
    }

    /// 借用完整报告进行校验, 成功时消费当前 Operation 并返回 Ready.
    ///
    /// 调用者保证报告对应已结束的 Attempt, 结果是否满足请求由外层判断.
    ///
    /// # Errors
    ///
    /// 依次检查 Operation 与 Attempt 身份, 拒绝时错误保留原 Operation.
    pub fn accept_report<R, V>(
        self,
        report: &Report<O, A, R, V>,
    ) -> Result<Operation<O, P>, crate::AcceptReportError<Self>>
    where
        O: Eq,
        A: Eq,
    {
        if &self.id != report.operation_id() {
            return Err(crate::AcceptReportError::OperationMismatch { operation: self });
        }
        if &self.state.attempt_id != report.attempt_id() {
            return Err(crate::AcceptReportError::AttemptMismatch { operation: self });
        }

        Ok(Operation {
            id: self.id,
            request: self.request,
            state: Ready {
                machine: lifecycle::transition(self.state.machine.accept_report()),
            },
        })
    }
}

impl<O, P> Operation<O, P, Closed> {
    /// 消费已关闭的 Operation, 幂等返回 Closed.
    pub fn close(self) -> Self {
        Self {
            id: self.id,
            request: self.request,
            state: Closed {
                machine: lifecycle::transition(self.state.machine.close()),
            },
        }
    }
}
