//! 在调用方提供的 Authority 与 Worker 之间进行本地类型化交付

use crate::{
    Authority, Worker,
    executor::ExecutorError,
    id::{AttemptId, OperationId, ReportId, RoleInstanceId},
    memory::{Admission, Limits, OperationView, StateError, Submission},
    model::{Delivery, DispatchAttempt, Operation, SubmitReport},
};
use std::future::Future;
use thiserror::Error;

/// 可选的本地执行组合, 在独立 Authority 与 Worker 之间交付授权和证据
///
/// 使用 [`Self::from_parts`] 组装调用方创建的角色, 或用 [`Self::new`] 创建内存角色.
/// 本对象不建立网络, 数据库或遥测连接; 执行由调用方轮询 [`Self::next_report`] 驱动.
/// 丢弃组合会丢弃内存状态并中止 Worker 尚未完成的本地任务;
/// 使用 [`Self::into_parts`] 可以保留角色与执行状态, 改由调用方继续驱动.
///
/// # 示例
///
/// ```
/// use beamlet::{Authority, LocalExecution, Worker, memory::Limits};
///
/// let authority = Authority::<String, String, String>::new(
///     1.into(), vec![2.into()], Limits::default(),
/// )?;
/// let worker = Worker::new(1.into(), vec![2.into()], Limits::default())?;
/// let execution = LocalExecution::from_parts(authority, worker);
/// let (authority, worker) = execution.into_parts();
/// # Ok::<(), beamlet::memory::StateError>(())
/// ```
#[derive(Debug)]
pub struct LocalExecution<P, O, E> {
    authority: Authority<P, O, E>,
    worker: Worker<P, O, E>,
}

impl<P, O, E> LocalExecution<P, O, E> {
    /// 接管调用方显式提供的两个角色, 保留其原有状态与资源上限
    ///
    /// 此操作只组合已有角色, 不派发或补发已有授权, 也不启动执行.
    /// 调用方应提供相互绑定的角色; 每次授权和 Report 仍在交付时校验实例绑定,
    /// 错配通过提交或结果收录错误报告, 不重写传入角色的身份或历史.
    pub fn from_parts(authority: Authority<P, O, E>, worker: Worker<P, O, E>) -> Self {
        Self { authority, worker }
    }

    /// 取回原角色及其全部状态, 不取消 Worker 中已经开始的执行
    ///
    /// 调用方接管后续驱动与证据交付责任; 此操作不会自动补发或完成在途工作.
    pub fn into_parts(self) -> (Authority<P, O, E>, Worker<P, O, E>) {
        (self.authority, self.worker)
    }
}

impl<P, O, E> LocalExecution<P, O, E>
where
    P: Clone + Eq + Send + 'static,
    O: Clone + Eq + Send + 'static,
    E: Clone + Eq + Send + 'static,
{
    /// 按显式实例配置创建两个内存角色, 使用相同上限完成本地组装
    ///
    /// 需要分别配置角色上限或保留已有状态时, 使用 [`Self::from_parts`].
    /// 构造不启动任务, 不要求 Tokio 运行时.
    ///
    /// # 错误
    /// 实例配置或资源上限无效时返回错误
    pub fn new(
        instance: RoleInstanceId,
        workers: Vec<RoleInstanceId>,
        limits: Limits,
    ) -> Result<Self, LocalExecutionError> {
        let authority = Authority::new(instance, workers.clone(), limits)?;
        let worker = Worker::new(instance, workers, limits)?;
        Ok(Self::from_parts(authority, worker))
    }

    /// 将调用方意图关联到唯一授权, 并仅为首次接纳尝试派发
    ///
    /// # 错误
    /// 身份冲突或容量不足时返回错误. 若 Authority 已接纳而 Worker 拒绝入队,
    /// 请求仍保留为 Accepted; 不回滚事实, 也不自动重投或新建授权
    pub fn submit(
        &mut self,
        operation: Operation<P>,
        attempt: AttemptId,
    ) -> Result<Submission<P, O, E>, LocalExecutionError> {
        let id = operation.operation_id;
        let accepted = self.authority.submit(operation, attempt)?;
        if accepted.admission == Admission::New {
            self.worker.receive_dispatch(accepted.dispatch.clone())?;
            self.authority.mark_dispatched(id)?;
        }
        Ok(accepted)
    }

    /// 启动可运行任务并收录下一份证据; 没有在途任务时返回空值
    ///
    /// 取消本次等待不会取消已经启动的执行. 执行函数是直接传入的普通函数,
    /// 不建立能力描述, 注册表或动态处理器接口
    ///
    /// # 错误
    /// 驱动异常或证据无法采用时返回错误; 不转换成普通任务失败或重新执行
    ///
    /// # 恐慌
    /// 有任务可启动时须在 Tokio 运行时内调用; 执行函数构造过程恐慌时终止.
    pub async fn next_report<F, Fut>(
        &mut self,
        execute: F,
    ) -> Result<Option<Delivery<P, O, E>>, LocalExecutionError>
    where
        F: Fn(DispatchAttempt<P>) -> Fut,
        Fut: Future<Output = SubmitReport<O, E>> + Send + 'static,
    {
        let Some(report) = self.worker.next_report(execute).await? else {
            return Ok(None);
        };
        self.authority.receive_report(report.clone())?;
        Ok(Some(report))
    }

    /// 按稳定 OperationId 查询当前观察, 不暴露可变底层状态
    pub fn operation(&self, id: OperationId) -> Option<&OperationView<P, O, E>> {
        self.authority.operation(id)
    }

    /// 查询已经由 Authority 保留的执行证据
    pub fn report(&self, id: ReportId) -> Option<&Delivery<P, O, E>> {
        self.authority.report(id)
    }

    /// 只标记等待超时, 不取消执行或产生第二次授权
    ///
    /// # 错误
    /// 未保留该请求时返回错误
    pub fn mark_wait_timeout(&mut self, id: OperationId) -> Result<(), LocalExecutionError> {
        self.authority.mark_wait_timeout(id)?;
        Ok(())
    }
}

/// 应用用例错误, 独立于执行结果中的业务错误
#[derive(Debug, Error)]
pub enum LocalExecutionError {
    /// 接纳, 派发或证据关联不满足要求
    #[error(transparent)]
    State(#[from] StateError),
    /// 后台执行驱动异常
    #[error(transparent)]
    Executor(#[from] ExecutorError),
}
