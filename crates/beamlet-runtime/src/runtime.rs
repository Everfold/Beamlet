use crate::{CreateOperationError, RuntimeError};
use beamlet_kernel::{AwaitingReport, Closed, Operation, OperationStatus};
use beamlet_primitives::{Attempt, Report};

#[derive(Debug)]
enum Managed<O, A, P> {
    Ready(Operation<O, P>),
    AwaitingReport(Operation<O, P, AwaitingReport<A>>),
    Closed(Operation<O, P, Closed>),
}
impl<O, A, P> Managed<O, A, P> {
    /// 统一借用各类型状态的不可变身份.
    fn id(&self) -> &O {
        match self {
            Self::Ready(o) => o.id(),
            Self::AwaitingReport(o) => o.id(),
            Self::Closed(o) => o.id(),
        }
    }
}

/// 仅可读取的在线 Operation 视图, 不暴露内部可变对象.
#[derive(Debug, Clone, Copy)]
pub struct OperationView<'a, O, A, P> {
    operation: &'a Managed<O, A, P>,
}
impl<O, A, P> OperationView<'_, O, A, P> {
    /// 读取 Operation 身份.
    pub fn id(&self) -> &O {
        self.operation.id()
    }
    /// 借用原始请求, 不复制业务数据.
    pub fn request(&self) -> &P {
        match self.operation {
            Managed::Ready(o) => o.request(),
            Managed::AwaitingReport(o) => o.request(),
            Managed::Closed(o) => o.request(),
        }
    }
    /// 读取当前状态标签.
    pub fn status(&self) -> OperationStatus {
        match self.operation {
            Managed::Ready(o) => o.status(),
            Managed::AwaitingReport(o) => o.status(),
            Managed::Closed(o) => o.status(),
        }
    }
    /// 仅等待状态返回当前授权身份.
    pub fn pending_attempt_id(&self) -> Option<&A> {
        match self.operation {
            Managed::AwaitingReport(o) => Some(o.pending_attempt_id()),
            _ => None,
        }
    }
}

/// 在线状态管理, 不执行 Worker 任务或自动重试.
///
/// O 为 Operation 身份, A 为 Attempt 身份, P 为请求. 查找使用线性比较,
/// 不要求身份实现 Hash. 关闭对象与已用授权身份保留到此 Runtime 被释放.
#[derive(Debug)]
pub struct Runtime<O, A, P> {
    operations: Vec<Managed<O, A, P>>,
    used_attempts: Vec<A>,
}
impl<O, A, P> Default for Runtime<O, A, P> {
    /// 创建空的在线管理器.
    fn default() -> Self {
        Self::new()
    }
}
impl<O, A, P> Runtime<O, A, P> {
    /// 创建独立运行实例, 不执行 IO.
    pub fn new() -> Self {
        Self {
            operations: Vec::new(),
            used_attempts: Vec::new(),
        }
    }
}
impl<O: Eq, A, P> Runtime<O, A, P> {
    /// 注册请求, 由 kernel 创建 Ready Operation.
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

    /// 查询只读视图, 不存在时返回 None.
    pub fn operation(&self, id: &O) -> Option<OperationView<'_, O, A, P>> {
        self.operations
            .iter()
            .find(|o| o.id() == id)
            .map(|operation| OperationView { operation })
    }

    /// 找到对应对象的位置, 保持身份比较集中.
    fn position(&self, id: &O) -> Result<usize, RuntimeError> {
        self.operations
            .iter()
            .position(|o| o.id() == id)
            .ok_or(RuntimeError::OperationNotFound)
    }

    /// 显式关闭 Ready, 重复关闭保持 Closed.
    ///
    /// # Errors
    ///
    /// 不存在或正在等待报告时拒绝, 原状态不变.
    pub fn close(&mut self, id: &O) -> Result<(), RuntimeError> {
        let index = self.position(id)?;
        if matches!(&self.operations[index], Managed::AwaitingReport(_)) {
            return Err(RuntimeError::AttemptPending);
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
impl<O: Eq + Clone, A: Eq + Clone, P> Runtime<O, A, P> {
    /// 在 Ready 上签发一次授权并保存等待状态, 交付由外部负责.
    ///
    /// # Errors
    ///
    /// 不存在, 等待中, 已关闭或授权身份重复时拒绝. 被拒绝的身份不会标记已使用.
    pub fn create_attempt(&mut self, id: &O, attempt_id: A) -> Result<Attempt<O, A>, RuntimeError> {
        let index = self.position(id)?;
        match &self.operations[index] {
            Managed::AwaitingReport(_) => return Err(RuntimeError::AttemptPending),
            Managed::Closed(_) => return Err(RuntimeError::OperationClosed),
            Managed::Ready(_) => {}
        }
        if self.used_attempts.contains(&attempt_id) {
            return Err(RuntimeError::AttemptIdReused);
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
impl<O: Eq, A: Eq, P> Runtime<O, A, P> {
    /// 根据 Report 的 Operation 身份路由, 由 kernel 校验当前授权.
    ///
    /// 业务失败结果仍可接受; 报告只借用, 不保存结果或自动关闭.
    ///
    /// # Errors
    ///
    /// 找不到 Operation, 非等待状态或报告关联错误时拒绝, 保留原对象.
    pub fn accept_report<R, V>(&mut self, report: &Report<O, A, R, V>) -> Result<(), RuntimeError> {
        let index = self.position(report.operation_id())?;
        match &self.operations[index] {
            Managed::Ready(_) => return Err(RuntimeError::NoPendingAttempt),
            Managed::Closed(_) => return Err(RuntimeError::OperationClosed),
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
                Err(RuntimeError::ReportMismatch)
            }
        }
    }
}
