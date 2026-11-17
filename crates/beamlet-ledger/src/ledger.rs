use crate::{CreateOperationError, LedgerError};
use beamlet_kernel::{AwaitingReport, Closed, Operation, OperationStatus};
use beamlet_primitives::{Attempt, Report};

/// 台账内部对不同生命周期 Operation 的统一存储表示。
///
/// # 泛型参数
///
/// - `O`：Operation 身份类型。
/// - `A`：Attempt 身份类型。
/// - `P`：原始请求类型。
#[derive(Debug)]
enum Managed<O, A, P> {
    /// 可创建新授权或结束操作的 Ready 对象。
    Ready(
        /// 保留原身份与请求的 Ready Operation。
        Operation<O, P>,
    ),
    /// 正在等待特定授权报告的 Operation。
    AwaitingReport(
        /// 携带当前待处理 Attempt 身份的等待对象。
        Operation<O, P, AwaitingReport<A>>,
    ),
    /// 已经结束且不可再签发授权的 Operation。
    Closed(
        /// 保留原身份与请求的 Closed Operation。
        Operation<O, P, Closed>,
    ),
}

impl<O, A, P> Managed<O, A, P> {
    /// 借用 Operation 的固定身份。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Managed<O, A, P>` 的共享引用。
    ///
    /// # 返回值
    ///
    /// 原 Operation 身份的引用，生命周期转换不改变它。
    fn id(&self) -> &O {
        match self {
            Self::Ready(o) => o.id(),
            Self::AwaitingReport(o) => o.id(),
            Self::Closed(o) => o.id(),
        }
    }
}

/// 仅可读取的在线 Operation 视图, 不暴露内部可变对象.
///
/// # 泛型参数
///
/// - `'a`：台账的借用生命周期。
/// - `O`：Operation 身份类型。
/// - `A`：Attempt 身份类型。
/// - `P`：原始请求类型。
#[derive(Debug, Clone, Copy)]
pub struct OperationView<'a, O, A, P> {
    /// 台账内部对象的共享借用，只读且不能比台账活得更久。
    operation: &'a Managed<O, A, P>,
}

impl<O, A, P> OperationView<'_, O, A, P> {
    /// 借用 Operation 的固定身份。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `OperationView<'_, O, A, P>` 的共享引用。
    ///
    /// # 返回值
    ///
    /// 原 Operation 身份的引用，生命周期转换不改变它。
    pub fn id(&self) -> &O {
        self.operation.id()
    }

    /// 借用 Operation 保存的原始业务请求。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `OperationView<'_, O, A, P>` 的共享引用。
    ///
    /// # 返回值
    ///
    /// 原请求的只读引用，不转移所有权。
    pub fn request(&self) -> &P {
        match self.operation {
            Managed::Ready(o) => o.request(),
            Managed::AwaitingReport(o) => o.request(),
            Managed::Closed(o) => o.request(),
        }
    }

    /// 查询台账中 Operation 的当前生命周期标签。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `OperationView<'_, O, A, P>` 的共享引用。
    ///
    /// # 返回值
    ///
    /// Ready、AwaitingReport 或 Closed 状态枚举。
    pub fn status(&self) -> OperationStatus {
        match self.operation {
            Managed::Ready(o) => o.status(),
            Managed::AwaitingReport(o) => o.status(),
            Managed::Closed(o) => o.status(),
        }
    }

    /// 查询台账视图当前是否正在等待一份授权报告。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `OperationView<'_, O, A, P>` 的共享引用。
    ///
    /// # 返回值
    ///
    /// 等待状态返回 `Some(&AttemptId)`；Ready 和 Closed 返回 `None`。
    pub fn pending_attempt_id(&self) -> Option<&A> {
        match self.operation {
            Managed::AwaitingReport(o) => Some(o.pending_attempt_id()),
            _ => None,
        }
    }
}

/// 一组 Operation 的执行台账, 维护当前状态与 Attempt 身份唯一性.
///
/// O 为 Operation 身份, A 为 Attempt 身份, P 为请求. 查找使用线性比较,
/// 不要求身份实现 Hash. 关闭对象与已用授权身份保留到此 Ledger 被释放.
///
/// # 泛型参数
///
/// - `O`：Operation 身份类型。
/// - `A`：Attempt 身份类型。
/// - `P`：原始请求类型。
#[derive(Debug)]
pub struct Ledger<O, A, P> {
    /// 全部已注册 Operation 的当前状态，保留关闭对象，不承诺归档。
    operations: Vec<Managed<O, A, P>>,
    /// 该台账已经签发过的 Attempt 身份，用于拒绝重复授权身份。
    used_attempts: Vec<A>,
}

impl<O, A, P> Default for Ledger<O, A, P> {
    /// 创建不含 Operation 和已用授权身份的空台账。
    ///
    /// # 返回值
    ///
    /// 独立的 Ledger，尚未记录任何操作或 Attempt。
    fn default() -> Self {
        Self::new()
    }
}

impl<O, A, P> Ledger<O, A, P> {
    /// 创建不含 Operation 和已用授权身份的空台账。
    ///
    /// # 返回值
    ///
    /// 独立的 Ledger，尚未记录任何操作或 Attempt。
    pub fn new() -> Self {
        Self {
            operations: Vec::new(),
            used_attempts: Vec::new(),
        }
    }
}

impl<O: Eq, A, P> Ledger<O, A, P> {
    /// 向台账插入一个全新 Ready Operation。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Ledger<O, A, P>` 的可变引用。
    /// - `id`：Operation 的固定身份，用于创建、恢复或查询对应操作。
    /// - `request`：创建 Operation 时的原始业务请求；状态转换保持其内容。
    ///
    /// # 返回值
    ///
    /// 插入成功返回 `Ok(())`；身份已存在时返回携带输入身份和请求的 CreateOperationError。
    ///
    /// # Errors
    ///
    /// 身份重复时返回携带原身份和请求的错误, 不覆盖已有数据.
    pub fn create_operation(
        &mut self,
        id: O,
        request: P,
    ) -> Result<(), CreateOperationError<O, P>> {
        if self.operations.iter().any(|o| o.id() == &id) {
            return Err(CreateOperationError { id, request });
        }

        self.operations
            .push(Managed::Ready(Operation::new(id, request)));

        Ok(())
    }

    /// 按身份查询台账中的只读 Operation 视图。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Ledger<O, A, P>` 的共享引用。
    /// - `id`：Operation 的固定身份，用于创建、恢复或查询对应操作。
    ///
    /// # 返回值
    ///
    /// 存在时返回 `Some(OperationView)`，不存在时返回 `None`。
    pub fn operation(&self, id: &O) -> Option<OperationView<'_, O, A, P>> {
        self.operations
            .iter()
            .find(|o| o.id() == id)
            .map(|operation| OperationView { operation })
    }

    /// 定位 Operation 在内部存储中的索引。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Ledger<O, A, P>` 的共享引用。
    /// - `id`：Operation 的固定身份，用于创建、恢复或查询对应操作。
    ///
    /// # 返回值
    ///
    /// 成功返回零基位置；不存在返回 OperationNotFound。
    fn position(&self, id: &O) -> Result<usize, LedgerError> {
        self.operations
            .iter()
            .position(|o| o.id() == id)
            .ok_or(LedgerError::OperationNotFound)
    }

    /// 关闭台账中的 Ready Operation，已关闭时保持幂等。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Ledger<O, A, P>` 的可变引用。
    /// - `id`：Operation 的固定身份，用于创建、恢复或查询对应操作。
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`；不存在或仍等待报告时返回 LedgerError。
    ///
    /// # Errors
    ///
    /// 不存在或正在等待报告时拒绝, 原状态不变.
    pub fn close(&mut self, id: &O) -> Result<(), LedgerError> {
        let index = self.position(id)?;
        if matches!(&self.operations[index], Managed::AwaitingReport(_)) {
            return Err(LedgerError::AttemptPending);
        }

        let closed = match self.operations.remove(index) {
            Managed::Ready(operation) => operation.close(),
            Managed::Closed(operation) => operation.close(),
            Managed::AwaitingReport(_) => unreachable!("等待状态已在移出前拒绝"),
        };
        self.operations.insert(index, Managed::Closed(closed));

        Ok(())
    }
}

impl<O: Eq + Clone, A: Eq + Clone, P> Ledger<O, A, P> {
    /// 为一个 Ready Operation 签发台账内未使用过的授权身份。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Ledger<O, A, P>` 的可变引用。
    /// - `id`：Operation 的固定身份，用于创建、恢复或查询对应操作。
    /// - `attempt_id`：为本次授权分配的身份；Ledger 额外拒绝全台账已使用过的身份。
    ///
    /// # 返回值
    ///
    /// 成功返回 Attempt 并进入等待状态；状态非法或身份已用时返回 LedgerError。
    ///
    /// # Errors
    ///
    /// 不存在, 等待中, 已关闭或授权身份重复时拒绝. 被拒绝的身份不会标记已使用.
    pub fn create_attempt(&mut self, id: &O, attempt_id: A) -> Result<Attempt<O, A>, LedgerError> {
        let index = self.position(id)?;
        match &self.operations[index] {
            Managed::AwaitingReport(_) => return Err(LedgerError::AttemptPending),
            Managed::Closed(_) => return Err(LedgerError::OperationClosed),
            Managed::Ready(_) => {}
        }
        if self.used_attempts.contains(&attempt_id) {
            return Err(LedgerError::AttemptIdReused);
        }

        let used_id = attempt_id.clone();
        let Managed::Ready(operation) = self.operations.remove(index) else {
            unreachable!("只有 Ready 能进入授权创建")
        };
        let (operation, attempt) = operation.create_attempt(attempt_id);
        self.operations
            .insert(index, Managed::AwaitingReport(operation));
        self.used_attempts.push(used_id);

        Ok(attempt)
    }
}

impl<O: Eq, A: Eq, P> Ledger<O, A, P> {
    /// 用完整报告推进台账中对应 Operation 的生命周期。
    ///
    /// 业务失败结果仍可接受; 报告只借用, 不保存结果或自动关闭.
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Ledger<O, A, P>` 的可变引用。
    /// - `report`：完整报告，包含 Operation/Attempt 关联、报告身份及业务结果。
    ///
    /// # 返回值
    ///
    /// 校验成功返回 `Ok(())` 并转回 Ready；不存在、状态非法或关联错误返回 LedgerError，保持原对象。
    ///
    /// # Errors
    ///
    /// 找不到 Operation, 非等待状态或报告关联错误时拒绝, 保留原对象.
    pub fn accept_report<R, V>(&mut self, report: &Report<O, A, R, V>) -> Result<(), LedgerError> {
        let index = self.position(report.operation_id())?;
        match &self.operations[index] {
            Managed::Ready(_) => return Err(LedgerError::NoPendingAttempt),
            Managed::Closed(_) => return Err(LedgerError::OperationClosed),
            Managed::AwaitingReport(_) => {}
        }

        let Managed::AwaitingReport(operation) = self.operations.remove(index) else {
            unreachable!("只有等待状态能进入报告接受")
        };

        match operation.accept_report(report) {
            Ok(operation) => {
                self.operations.insert(index, Managed::Ready(operation));
                Ok(())
            }
            Err(error) => {
                self.operations
                    .insert(index, Managed::AwaitingReport(error.into_operation()));
                Err(LedgerError::ReportMismatch)
            }
        }
    }
}
