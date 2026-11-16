use crate::{Attempt, Report, lifecycle};

/// 生命周期的只读标签, 等待身份通过 pending_attempt_id 单独读取.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationStatus {
    /// 可以创建 Attempt 或关闭.
    Ready,
    /// 正在等待指定 Attempt 的报告.
    AwaitingReport,
    /// 已结束.
    Closed,
}

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
    /// 导出 Ready 标签, 不生成持久化记录.
    pub fn status(&self) -> OperationStatus {
        OperationStatus::Ready
    }

    /// 从已校验的 Closed 持久化状态重建对象, 不提交关闭事实.
    ///
    /// 调用者负责确认日志与快照的一致性.
    pub fn restore_closed(id: O, request: P) -> Operation<O, P, Closed> {
        Self::new(id, request).close()
    }

    /// 创建 Ready 对象或从已校验的 Ready 快照重建, 不执行 IO.
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
        let attempt = Attempt::from_parts(self.id.clone(), attempt_id.clone());
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
    /// 从已校验的等待状态重建对象, 不创建或交付新授权.
    ///
    /// 调用者负责确认快照与历史一致; 此处只验证授权所属身份.
    ///
    /// # Errors
    ///
    /// 授权所属 Operation 不匹配时, 错误归还所有输入数据.
    pub fn restore_awaiting_report(
        id: O,
        request: P,
        attempt: Attempt<O, A>,
    ) -> Result<Self, crate::RestoreOperationError<O, P, A>>
    where
        O: Eq,
    {
        if &id != attempt.operation_id() {
            return Err(crate::RestoreOperationError {
                id,
                request,
                attempt,
            });
        }

        let (_, attempt_id) = attempt.into_parts();
        Ok(Operation {
            id,
            request,
            state: AwaitingReport {
                machine: lifecycle::transition(lifecycle::Lifecycle::new(()).create_attempt()),
                attempt_id,
            },
        })
    }

    /// 导出等待标签, 等待身份通过 pending_attempt_id 读取.
    pub fn status(&self) -> OperationStatus {
        OperationStatus::AwaitingReport
    }

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
    /// 导出 Closed 标签, 不产生重复关闭记录.
    pub fn status(&self) -> OperationStatus {
        OperationStatus::Closed
    }

    /// 已关闭对象直接返回自身, 不再次触发转换或终态进入行为.
    pub fn close(self) -> Self {
        debug_assert!(self.state.machine.is_finished());
        self
    }
}
