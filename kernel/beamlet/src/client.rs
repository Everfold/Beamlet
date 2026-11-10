//! 显式绑定现有内存角色的 Client, 提交与查询不启动执行或基础设施

use crate::{
    Authority, Worker,
    id::{AttemptId, OperationId, ReportId, RoleInstanceId, SubmissionKey},
    local_execution::deliver_new,
    memory::{Admission, OperationView, StateError},
    model::{Delivery, Message, SubmitReport},
    state::{OperationPhase, WaitStatus},
};
use thiserror::Error;

/// 本次执行的原始身份关联, 运行实例不等于可跨重启使用的稳定 AuthorityId
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionBinding {
    /// 接纳请求的 Authority 运行实例
    pub authority_instance_id: RoleInstanceId,
    /// 原授权指定的 Worker 运行实例
    pub worker_instance_id: RoleInstanceId,
    /// 原请求身份
    pub operation_id: OperationId,
    /// 原执行授权身份
    pub attempt_id: AttemptId,
}

impl ExecutionBinding {
    fn from_dispatch<P, O, E>(delivery: &Delivery<P, O, E>) -> Self {
        let Message::DispatchAttempt(attempt) = &delivery.message else {
            unreachable!("Authority 的请求记录必须包含 Attempt");
        };
        Self {
            authority_instance_id: delivery.source_instance_id,
            worker_instance_id: delivery.target_instance_id,
            operation_id: attempt.operation.operation_id,
            attempt_id: attempt.attempt_id,
        }
    }
}

/// Authority 已知事实的独立快照, 修改或丢弃它不会修改账本
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct OperationObservation<P> {
    /// 原执行身份关联
    pub binding: ExecutionBinding,
    /// 完整不可变任务定义的副本
    pub task: P,
    /// Authority 的局部观察, 不推断 Worker 实际运行状态
    pub phase: OperationPhase,
    /// 已记录的等待观察, 超时不表示执行取消
    pub wait_status: WaitStatus,
    /// 首份采用证据的身份, 可通过 query_report 读取
    pub adopted_report: Option<ReportId>,
}

impl<P: Clone> OperationObservation<P> {
    fn from_view<O, E>(view: &OperationView<P, O, E>) -> Self {
        let Message::DispatchAttempt(attempt) = &view.dispatch.message else {
            unreachable!("Authority 的请求记录必须包含 Attempt");
        };
        Self {
            binding: ExecutionBinding::from_dispatch(&view.dispatch),
            task: attempt.operation.task.clone(),
            phase: view.phase,
            wait_status: view.wait_status,
            adopted_report: view.adopted_report,
        }
    }
}

/// 提交的接纳结果及当前观察, 不代表任务执行成功或持久化
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct SubmissionReceipt<P> {
    /// 调用方原提交键
    pub submission_key: SubmissionKey,
    /// 首次接纳或沿用原执行
    pub admission: Admission,
    /// 交付尝试后的 Authority 观察
    pub operation: OperationObservation<P>,
}

/// 已保留的完整 Report 副本, 保留原 Worker 到 Authority 的身份方向
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct ReportObservation<O, E> {
    /// 收录该证据的 Authority 运行实例
    pub authority_instance_id: RoleInstanceId,
    /// 提交该证据的 Worker 运行实例
    pub worker_instance_id: RoleInstanceId,
    /// 原始证据, 包括结果与独立影响结论
    pub report: SubmitReport<O, E>,
}

/// 本地提交或查询错误, 未找到记录与内容冲突有不同表示
#[derive(Debug, Error)]
pub enum LocalClientError {
    /// 当前内存账本没有此提交键, 不证明重启前未接纳
    #[error("当前 Authority 未保留提交键 {0}")]
    SubmissionNotFound(SubmissionKey),
    /// 当前内存账本没有此 Operation
    #[error("当前 Authority 未保留 Operation {0}")]
    OperationNotFound(OperationId),
    /// 当前内存账本没有此 Report
    #[error("当前 Authority 未保留 Report {0}")]
    ReportNotFound(ReportId),
    /// 本次提交未接纳, 同键内容冲突保留为 StateError::SubmissionConflict
    #[error(transparent)]
    Submission(#[from] StateError),
    /// 已接纳但本地交付失败; 可按原键找回执行, 不自动重投
    #[error("请求已接纳, 本地交付失败: {source}")]
    Dispatch {
        /// 已接纳的原执行身份
        binding: ExecutionBinding,
        /// 交付错误, 不等于任务失败
        #[source]
        source: StateError,
    },
}

/// 借用应用显式提供的 Authority, Worker 与身份生成函数的本地绑定
///
/// 构造和查询不分配执行 ID, 不创建线程, 网络, 数据库或任务.
/// 仅首次接纳会交付授权入队, Worker 的执行与 Report 收录由宿主继续驱动.
/// 释放 Client 后角色及其队列和运行任务仍由应用持有, 可以重新绑定查询.
/// 此入口用于可信同进程调用, 不提供远端认证, 通知或持久恢复.
///
/// # 示例
///
/// ```
/// use beamlet::{Authority, LocalClient, Worker, memory::Limits};
/// let mut authority = Authority::<String, String, String>::new(
///     1.into(), vec![2.into()], Limits::default(),
/// )?;
/// let mut worker = Worker::new(1.into(), vec![2.into()], Limits::default())?;
/// let mut client = LocalClient::new(&mut authority, &mut worker, || (10.into(), 20.into()));
/// let receipt = client.submit(7.into(), "hello".to_owned())?;
/// let recovered = client.query_submission(7.into())?;
/// assert_eq!(receipt.operation, recovered);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct LocalClient<'a, P, O, E, F> {
    authority: &'a mut Authority<P, O, E>,
    worker: &'a mut Worker<P, O, E>,
    allocate: F,
}

impl<'a, P, O, E, F> LocalClient<'a, P, O, E, F> {
    /// 绑定现有角色; 实例关系沿用角色配置, 交付时校验, 不改写身份
    pub fn new(
        authority: &'a mut Authority<P, O, E>,
        worker: &'a mut Worker<P, O, E>,
        allocate: F,
    ) -> Self {
        Self {
            authority,
            worker,
            allocate,
        }
    }
}

impl<P: Clone + Eq, O: Clone + Eq, E: Clone + Eq, F> LocalClient<'_, P, O, E, F> {
    /// 按原提交键找回当前观察, 不需要原定义或接纳回执
    ///
    /// # 错误
    /// 当前绑定的 Authority 不保留该键时返回 SubmissionNotFound
    pub fn query_submission(
        &self,
        key: SubmissionKey,
    ) -> Result<OperationObservation<P>, LocalClientError> {
        self.authority
            .submission(key)
            .map(OperationObservation::from_view)
            .ok_or(LocalClientError::SubmissionNotFound(key))
    }

    /// 按 OperationId 读取当前观察, 返回值不借用或修改账本
    ///
    /// # 错误
    /// 当前绑定的 Authority 不保留该请求时返回 OperationNotFound
    pub fn query_operation(
        &self,
        id: OperationId,
    ) -> Result<OperationObservation<P>, LocalClientError> {
        self.authority
            .operation(id)
            .map(OperationObservation::from_view)
            .ok_or(LocalClientError::OperationNotFound(id))
    }

    /// 读取完整已保留证据, 包括未被采用为代表结果的附加 Report
    ///
    /// # 错误
    /// 当前绑定的 Authority 不保留该证据时返回 ReportNotFound
    pub fn query_report(&self, id: ReportId) -> Result<ReportObservation<O, E>, LocalClientError> {
        let delivery = self
            .authority
            .report(id)
            .ok_or(LocalClientError::ReportNotFound(id))?;
        let Message::SubmitReport(report) = &delivery.message else {
            unreachable!("Authority 的证据记录必须包含 Report");
        };
        Ok(ReportObservation {
            authority_instance_id: delivery.target_instance_id,
            worker_instance_id: delivery.source_instance_id,
            report: report.clone(),
        })
    }
}

impl<P, O, E, F> LocalClient<'_, P, O, E, F>
where
    P: Clone + Eq + Send + 'static,
    O: Clone + Eq + Send + 'static,
    E: Clone + Eq + Send + 'static,
    F: FnMut() -> (OperationId, AttemptId),
{
    /// 按原键接纳并尝试本地交付, 返回结构化观察, 不执行任务
    ///
    /// 仅新提交调用 ID 生成函数; 重复提交返回当前观察且不重新交付.
    ///
    /// # 错误
    /// 内容冲突, ID 冲突或容量不足返回 Submission; 接纳后的交付错误返回 Dispatch,
    /// 其中保留已接纳执行的绑定. 两种错误均不创建失败 Report 或自动重试.
    pub fn submit(
        &mut self,
        key: SubmissionKey,
        task: P,
    ) -> Result<SubmissionReceipt<P>, LocalClientError> {
        let accepted = self.authority.submit_keyed(key, task, &mut self.allocate)?;
        let binding = ExecutionBinding::from_dispatch(&accepted.dispatch);
        deliver_new(self.authority, self.worker, &accepted)
            .map_err(|source| LocalClientError::Dispatch { binding, source })?;
        Ok(SubmissionReceipt {
            submission_key: key,
            admission: accepted.admission,
            operation: self.query_operation(binding.operation_id)?,
        })
    }
}
