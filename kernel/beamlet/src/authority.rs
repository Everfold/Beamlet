//! 独立 Authority 入口, 只接纳请求与收录证据, 不持有执行器

use crate::{
    OperationObservation, ReportObservation,
    id::{AttemptId, OperationId, ReportId, RoleInstanceId, SubmissionKey},
    memory::{
        Authority as Ledger, Limits, OperationView, ReportDisposition, StateError, Submission,
    },
    model::{Delivery, Message, Operation},
    observe::Notice,
    query,
};

/// Authority 的应用入口, 将请求接纳与消息交付分开
///
/// 返回授权不表示已经发送, 调用方负责交付并记录派发观察
/// 所有记录仅在内存中保留, 不提供可靠重投或重启恢复
#[derive(Debug)]
pub struct Authority<P, O, E> {
    instance: RoleInstanceId,
    ledger: Ledger<P, O, E>,
}

impl<P: Clone + Eq, O: Clone + Eq, E: Clone + Eq> Authority<P, O, E> {
    /// 使用静态 Worker 实例列表创建独立 Authority
    ///
    /// # 错误
    /// 资源上限无效, Worker 列表为空或实例身份重复时返回错误
    pub fn new(
        instance: RoleInstanceId,
        workers: Vec<RoleInstanceId>,
        limits: Limits,
    ) -> Result<Self, StateError> {
        Ok(Self {
            instance,
            ledger: Ledger::new(instance, workers, limits)?,
        })
    }

    /// 接纳本地应用请求并返回待交付授权, 不自动发送或接触 Worker
    ///
    /// 重复提交只返回原有授权, 不表示应当自动重新交付
    ///
    /// # 错误
    /// 身份冲突或记录容量不足时返回错误, 不改变已保留事实
    pub fn submit(
        &mut self,
        operation: Operation<P>,
        attempt: AttemptId,
    ) -> Result<Submission<P, O, E>, StateError> {
        self.ledger.submit(
            Delivery::new(
                self.instance,
                self.instance,
                Message::SubmitOperation(operation),
            ),
            attempt,
        )
    }

    /// 校验提交键与完整定义, 返回已接纳执行的当前观察
    ///
    /// 未知键返回空值, 不分配身份或预留资格. 供调用方在接纳前检查交付资源;
    /// 提交时仍会再次校验. 传输版本与临时回复地址不属于任务定义.
    ///
    /// # 错误
    /// 同键不同定义时返回 SubmissionConflict
    pub fn check_submission(
        &self,
        key: SubmissionKey,
        task: &P,
    ) -> Result<Option<&OperationView<P, O, E>>, StateError> {
        self.ledger.check_submission(key, task)
    }

    /// 按原提交键读取当前观察, 不重送定义, 分配身份或派发
    ///
    /// 空值只表示当前内存账本没有此键, 不证明其他实例或重启前未接纳.
    pub fn submission(&self, key: SubmissionKey) -> Option<&OperationView<P, O, E>> {
        self.ledger.submission(key)
    }

    /// 按提交键接纳完整定义, 仅在创建新执行时调用应用提供的身份生成函数
    ///
    /// 接纳成功前同步保存键映射, Operation 与 Attempt; 不执行或派发任务.
    /// 重复键沿用原授权, 内容冲突与容量不足时不调用 allocate.
    /// 分配的 ID 冲突时拒绝, 不自动重新生成; 候选 ID 不要求回收.
    /// 此保证仅限当前内存账本, 不提供持久性或跨实例去重.
    ///
    /// # 示例
    ///
    /// ```
    /// use beamlet::{Authority, memory::{Admission, Limits}};
    /// let mut authority = Authority::<String, String, String>::new(
    ///     1.into(), vec![2.into()], Limits::default(),
    /// )?;
    /// let accepted = authority.submit_keyed(7.into(), "hello".to_owned(), || {
    ///     (10.into(), 20.into()) // 应用选择身份生成机制
    /// })?;
    /// assert_eq!(accepted.admission, Admission::New);
    /// # Ok::<(), beamlet::memory::StateError>(())
    /// ```
    ///
    /// # 错误
    /// 提交内容冲突, 记录容量不足或生成的身份已被使用时返回错误
    pub fn submit_keyed<F>(
        &mut self,
        key: SubmissionKey,
        task: P,
        allocate: F,
    ) -> Result<Submission<P, O, E>, StateError>
    where
        F: FnOnce() -> (OperationId, AttemptId),
    {
        self.ledger.submit_keyed(key, task, allocate)
    }

    /// 调用方确认已发出后记录派发观察, 不代表 Worker 已接纳
    ///
    /// # 错误
    /// 请求尚未被接纳时返回错误
    pub fn mark_dispatched(&mut self, operation: OperationId) -> Result<(), StateError> {
        self.ledger.mark_dispatched(operation)
    }

    /// 校验并保留收到的 Report, 不因等待超时而丢弃迟到证据
    ///
    /// # 错误
    /// 消息, 实例绑定, 身份, 证据或记录容量不合法时返回错误
    pub fn receive_report(
        &mut self,
        report: Delivery<P, O, E>,
    ) -> Result<ReportDisposition, StateError> {
        self.ledger.receive_report(report)
    }

    /// 查询请求的当前观察, 不访问 Worker 的状态
    pub fn operation(&self, operation: OperationId) -> Option<&OperationView<P, O, E>> {
        self.ledger.operation(operation)
    }

    /// 查询已保留的证据, 包括未被采用为结果的 Report
    pub fn report(&self, report: ReportId) -> Option<&Delivery<P, O, E>> {
        self.ledger.report(report)
    }

    /// 从已保留证据构造通知, 不改变账本或承诺通知交付
    pub fn report_notice(&self, id: ReportId) -> Option<Notice<P, O, E>> {
        let report = ReportObservation::from_delivery(self.report(id)?);
        let operation =
            OperationObservation::from_view(self.operation(report.report.operation_id)?);
        Some(Notice { operation, report })
    }

    /// 只读回答明确指向本实例的查询, 不分配身份, 派发, 修改等待状态或接触 Worker
    ///
    /// # 错误
    /// 目标不是本实例时返回 WrongAuthority, 不冒充目标账本回答 NotFound
    pub fn answer_query(
        &self,
        request: query::Request,
    ) -> Result<query::Reply<P, O, E>, query::WrongAuthority> {
        if request.authority_instance_id != self.instance {
            return Err(query::WrongAuthority);
        }
        Ok(query::Reply {
            request,
            observation: query::observe(self, request.selection),
        })
    }

    /// 只标记等待超时, 不取消执行或产生新授权
    ///
    /// # 错误
    /// 请求尚未被接纳时返回错误
    pub fn mark_wait_timeout(&mut self, operation: OperationId) -> Result<(), StateError> {
        self.ledger.mark_wait_timeout(operation)
    }
}
