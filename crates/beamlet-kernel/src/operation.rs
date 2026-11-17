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
    /// 底层状态机的 Ready 类型状态，保证只有合法操作可调用。
    machine: lifecycle::Lifecycle<(), lifecycle::Ready>,
}

/// 等待指定 Attempt 的报告, 只能接受报告后继续推进.
///
/// # 泛型参数
///
/// - `A`：Attempt 身份类型。
#[derive(Debug)]
pub struct AwaitingReport<A> {
    /// 底层状态机的 AwaitingReport 类型状态。
    machine: lifecycle::Lifecycle<(), lifecycle::AwaitingReport>,
    /// 当前唯一等待报告的 Attempt 身份。
    attempt_id: A,
}

/// Operation 已结束, 允许重复关闭.
#[derive(Debug)]
pub struct Closed {
    /// 底层状态机的 Closed 终态。
    machine: lifecycle::Lifecycle<(), lifecycle::Closed>,
}

/// 身份与请求保持不变的 Operation, S 表达当前生命周期状态.
///
/// Ready 可签发或关闭；AwaitingReport 只接受匹配报告；Closed 不再执行。
///
/// # 泛型参数
///
/// - `O`：Operation 身份类型。
/// - `P`：原始请求类型。
/// - `S`：生命周期类型状态，默认 Ready。
#[derive(Debug)]
#[must_use]
pub struct Operation<O, P, S = Ready> {
    /// Operation 的固定身份，恢复或重复创建失败时仍保留输入所有权。
    id: O,
    /// Operation 的原始请求，不随生命周期转换改变。
    request: P,
    /// 当前生命周期的类型状态，决定可用的转换方法。
    state: S,
}

impl<O, P, S> Operation<O, P, S> {
    /// 借用 Operation 的固定身份。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Operation<O, P, S>` 的共享引用。
    ///
    /// # 返回值
    ///
    /// 原 Operation 身份的引用，生命周期转换不改变它。
    pub fn id(&self) -> &O {
        &self.id
    }

    /// 借用 Operation 保存的原始业务请求。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Operation<O, P, S>` 的共享引用。
    ///
    /// # 返回值
    ///
    /// 原请求的只读引用，不转移所有权。
    pub fn request(&self) -> &P {
        &self.request
    }
}

impl<O, P> Operation<O, P> {
    /// 查询 Ready Operation 的状态标签。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Operation<O, P>` 的共享引用。
    ///
    /// # 返回值
    ///
    /// `OperationStatus::Ready`。
    pub fn status(&self) -> OperationStatus {
        OperationStatus::Ready
    }

    /// 按已持久化的身份和请求重建已关闭 Operation。
    ///
    /// 调用者负责确认日志与快照的一致性.
    ///
    /// # 参数
    ///
    /// - `id`：Operation 的固定身份，用于创建、恢复或查询对应操作。
    /// - `request`：创建 Operation 时的原始业务请求；状态转换保持其内容。
    ///
    /// # 返回值
    ///
    /// Closed 状态的 Operation，不重新触发关闭动作。
    pub fn restore_closed(id: O, request: P) -> Operation<O, P, Closed> {
        Self::new(id, request).close()
    }

    /// 使用身份和原始请求创建 Ready Operation。
    ///
    /// # 参数
    ///
    /// - `id`：Operation 的固定身份，用于创建、恢复或查询对应操作。
    /// - `request`：创建 Operation 时的原始业务请求；状态转换保持其内容。
    ///
    /// # 返回值
    ///
    /// 处于 Ready 的 Operation，尚未签发 Attempt。
    pub fn new(id: O, request: P) -> Self {
        Self {
            id,
            request,
            state: Ready {
                machine: lifecycle::Lifecycle::new(()),
            },
        }
    }

    /// 消费 Ready Operation 并签发一个新的 Attempt。
    ///
    /// 调用者提供未使用的 Attempt 身份. 两个身份各克隆一次供授权独立持有.
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Operation<O, P>` 的值；调用后转移其所有权（Copy 类型按值复制）。
    /// - `attempt_id`：为本次授权分配的身份；Ledger 额外拒绝全台账已使用过的身份。
    ///
    /// # 返回值
    ///
    /// 二元组 `(等待报告的 Operation, Attempt)`；二者使用同一 Operation/Attempt 身份。
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

    /// 把 Ready Operation 转换为 Closed。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Operation<O, P>` 的值；调用后转移其所有权（Copy 类型按值复制）。
    ///
    /// # 返回值
    ///
    /// 保留身份和请求的 Closed Operation。
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
    /// 根据已保存的授权重建等待报告的 Operation。
    ///
    /// 调用者负责确认快照与历史一致; 此处只验证授权所属身份.
    ///
    /// # 参数
    ///
    /// - `id`：Operation 的固定身份，用于创建、恢复或查询对应操作。
    /// - `request`：创建 Operation 时的原始业务请求；状态转换保持其内容。
    /// - `attempt`：恢复时已保存的授权，其 operation_id 必须与 id 一致。
    ///
    /// # 返回值
    ///
    /// 成功返回 AwaitingReport Operation；授权所属 Operation 不符时返回携带全部输入的错误。
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

    /// 查询等待报告 Operation 的状态标签。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Operation<O, P, AwaitingReport<A>>` 的共享引用。
    ///
    /// # 返回值
    ///
    /// `OperationStatus::AwaitingReport`。
    pub fn status(&self) -> OperationStatus {
        OperationStatus::AwaitingReport
    }

    /// 借用当前等待报告的 Attempt 身份。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Operation<O, P, AwaitingReport<A>>` 的共享引用。
    ///
    /// # 返回值
    ///
    /// 当前授权身份的引用，不创建替代授权。
    pub fn pending_attempt_id(&self) -> &A {
        &self.state.attempt_id
    }

    /// 校验完整报告的关联并将 Operation 转回 Ready。
    ///
    /// 调用者保证报告对应已结束的 Attempt, 结果是否满足请求由外层判断.
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Operation<O, P, AwaitingReport<A>>` 的值；调用后转移其所有权（Copy 类型按值复制）。
    /// - `report`：完整报告，包含 Operation/Attempt 关联、报告身份及业务结果。
    ///
    /// # 返回值
    ///
    /// 成功返回 Ready Operation；关联不符时错误保留原等待对象。
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
    /// 查询已关闭 Operation 的状态标签。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Operation<O, P, Closed>` 的共享引用。
    ///
    /// # 返回值
    ///
    /// `OperationStatus::Closed`。
    pub fn status(&self) -> OperationStatus {
        OperationStatus::Closed
    }

    /// 对已经关闭的 Operation 幂等关闭。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Operation<O, P, Closed>` 的值；调用后转移其所有权（Copy 类型按值复制）。
    ///
    /// # 返回值
    ///
    /// 原 Closed Operation，自身状态与身份不变。
    pub fn close(self) -> Self {
        debug_assert!(self.state.machine.is_finished());
        self
    }
}
