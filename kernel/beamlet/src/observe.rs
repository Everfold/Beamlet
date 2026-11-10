//! 单个执行的单调观察与有界结果等待, 不拥有连接或执行权

use crate::{
    ExecutionBinding, OperationObservation, ReportObservation,
    id::{OperationId, ReportId, RoleInstanceId},
    query,
    state::{OperationPhase, WaitStatus},
};
use std::{collections::HashMap, error::Error as StdError, future::Future, time::Duration};
use thiserror::Error;

/// Authority 收录后产生的事实, 保留采用决定及原 Worker 到 Authority 的证据方向
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Notice<P, O, E> {
    /// 生成通知时的执行快照, adopted_report 不一定是本通知携带的附加证据
    pub operation: OperationObservation<P>,
    /// 已收录的完整证据
    pub report: ReportObservation<O, E>,
}

impl<P, O, E> Notice<P, O, E> {
    /// 校验内部关联和证据, 不代替发送方认证
    pub fn is_valid(&self) -> bool {
        query::valid_operation(&self.operation)
            && self.operation.phase == OperationPhase::Reported
            && binding(&self.report) == self.operation.binding
            && self.report.report.validate().is_ok()
    }
}

/// 合并产生的新事实; 重复或较旧观察返回默认空变化
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Change {
    /// 执行快照有新增事实
    pub operation_changed: bool,
    /// 首次保留的 Report, 同 ID 不重复产生
    pub new_report: Option<ReportId>,
}

/// 合并失败不改变已有观察, 不触发重新提交或执行
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum MergeError {
    /// 身份, 状态或证据不合法
    #[error("观察与原执行不匹配或证据无效")]
    Invalid,
    /// 不可变绑定, 任务, 采用决定或同 ID Report 内容冲突
    #[error("观察与已保留事实冲突")]
    Conflict,
    /// 报告保留上限已到, 不删除旧去重记录
    #[error("观察的 Report 保留上限已到")]
    Capacity,
}

/// 可跨 Client 重新绑定保留的内存观察; 不表示 Agent 已持久消费结果
#[derive(Debug)]
pub struct State<P, O, E> {
    authority: RoleInstanceId,
    operation_id: OperationId,
    operation: Option<OperationObservation<P>>,
    reports: HashMap<ReportId, ReportObservation<O, E>>,
    max_reports: usize,
    delivered: bool,
}

impl<P, O, E> State<P, O, E> {
    /// 跟踪原执行并显式限制证据数量, 不建立基础设施
    /// # 错误
    /// max_reports 为零时返回 Capacity
    pub fn new(
        authority: RoleInstanceId,
        operation: OperationId,
        max_reports: usize,
    ) -> Result<Self, MergeError> {
        if max_reports == 0 {
            return Err(MergeError::Capacity);
        }
        Ok(Self {
            authority,
            operation_id: operation,
            operation: None,
            reports: HashMap::new(),
            max_reports,
            delivered: false,
        })
    }

    /// 原 Authority 运行实例, 重新绑定不切换账本
    pub fn authority(&self) -> RoleInstanceId {
        self.authority
    }
    /// 原 Operation 身份
    pub fn operation_id(&self) -> OperationId {
        self.operation_id
    }
    /// 已合并的单调快照
    pub fn operation(&self) -> Option<&OperationObservation<P>> {
        self.operation.as_ref()
    }
    /// 已保留的完整证据, 包括附加证据
    pub fn report(&self, id: ReportId) -> Option<&ReportObservation<O, E>> {
        self.reports.get(&id)
    }
    /// 采用的证据尚缺正文时需要补齐的原 ReportId
    pub fn missing_report(&self) -> Option<ReportId> {
        self.operation
            .as_ref()
            .and_then(|view| view.adopted_report)
            .filter(|id| !self.reports.contains_key(id))
    }
    /// 本对象是否已经向调用者返回过采用的报告
    pub fn delivered(&self) -> bool {
        self.delivered
    }
}

impl<P: Eq, O: Clone + Eq, E: Clone + Eq> State<P, O, E> {
    fn check_operation(&self, view: &OperationObservation<P>) -> Result<(), MergeError> {
        if view.binding.authority_instance_id != self.authority
            || view.binding.operation_id != self.operation_id
            || !query::valid_operation(view)
        {
            return Err(MergeError::Invalid);
        }
        if let Some(old) = &self.operation
            && (old.binding != view.binding
                || old.task != view.task
                || matches!((old.adopted_report, view.adopted_report), (Some(a), Some(b)) if a != b))
        {
            return Err(MergeError::Conflict);
        }
        if self
            .reports
            .values()
            .any(|report| binding(report) != view.binding)
        {
            return Err(MergeError::Conflict);
        }
        Ok(())
    }

    fn check_report(&self, view: &ReportObservation<O, E>) -> Result<(), MergeError> {
        if view.authority_instance_id != self.authority
            || view.report.operation_id != self.operation_id
            || view.report.validate().is_err()
        {
            return Err(MergeError::Invalid);
        }
        if self
            .operation
            .as_ref()
            .is_some_and(|operation| operation.binding != binding(view))
            || self
                .reports
                .values()
                .any(|old| binding(old) != binding(view))
        {
            return Err(MergeError::Conflict);
        }
        if let Some(old) = self.reports.get(&view.report.report_id) {
            if old != view {
                return Err(MergeError::Conflict);
            }
        } else if self.reports.len() >= self.max_reports {
            return Err(MergeError::Capacity);
        }
        Ok(())
    }

    /// 合并独立状态事实, 旧阶段及 Waiting 不覆盖已知 Reported 或 TimedOut
    /// # 错误
    /// 非法状态或不可变事实冲突时返回错误, 保留原观察
    pub fn merge_operation(
        &mut self,
        mut view: OperationObservation<P>,
    ) -> Result<Change, MergeError> {
        self.check_operation(&view)?;
        if let Some(old) = &self.operation {
            if rank(old.phase) > rank(view.phase) {
                view.phase = old.phase;
            }
            if old.wait_status == WaitStatus::TimedOut {
                view.wait_status = WaitStatus::TimedOut;
            }
            view.adopted_report = old.adopted_report.or(view.adopted_report);
            if old == &view {
                return Ok(Change::default());
            }
        }
        self.operation = Some(view);
        Ok(Change {
            operation_changed: true,
            new_report: None,
        })
    }

    /// 保留唯一证据; 单独收到 Report 不推断 Authority 采用了它
    /// # 错误
    /// 身份, 证据, 内容冲突或容量不足时返回错误
    pub fn merge_report(&mut self, report: ReportObservation<O, E>) -> Result<Change, MergeError> {
        self.check_report(&report)?;
        let id = report.report.report_id;
        if self.reports.contains_key(&id) {
            return Ok(Change::default());
        }
        self.reports.insert(id, report);
        Ok(Change {
            operation_changed: false,
            new_report: Some(id),
        })
    }

    /// 原子合并通知中的快照与完整证据, 失败不留下半份观察
    /// # 错误
    /// 非法通知, 冲突或容量不足时返回错误
    pub fn merge_notice(&mut self, notice: Notice<P, O, E>) -> Result<Change, MergeError> {
        if !notice.is_valid() {
            return Err(MergeError::Invalid);
        }
        self.check_operation(&notice.operation)?;
        self.check_report(&notice.report)?;
        let operation = self.merge_operation(notice.operation)?;
        let report = self.merge_report(notice.report)?;
        Ok(Change {
            operation_changed: operation.operation_changed,
            new_report: report.new_report,
        })
    }

    /// 每个 State 至多返回一次已采用的完整证据; 调用方另行持久化消费位置
    pub fn take_adopted_report(&mut self) -> Option<ReportObservation<O, E>> {
        if self.delivered {
            return None;
        }
        let id = self.operation.as_ref()?.adopted_report?;
        let report = self.reports.get(&id)?.clone();
        self.delivered = true;
        Some(report)
    }
}

fn rank(phase: OperationPhase) -> u8 {
    match phase {
        OperationPhase::Accepted => 0,
        OperationPhase::Dispatched => 1,
        OperationPhase::Reported => 2,
        _ => 0,
    }
}

fn binding<O, E>(view: &ReportObservation<O, E>) -> ExecutionBinding {
    ExecutionBinding {
        authority_instance_id: view.authority_instance_id,
        worker_instance_id: view.worker_instance_id,
        operation_id: view.report.operation_id,
        attempt_id: view.report.attempt_id,
    }
}

/// 由应用预先建立且独占消费的通知队列, recv 取消后不得丢弃未返回的消息
pub trait Notifications: Send + Sync {
    /// 通道故障, 不等于原执行失败
    type Error: StdError + Send + Sync + 'static;
    /// 接收下一条通知; None 表示过滤一条非法传输消息
    fn recv(&self) -> impl Future<Output = Result<Option<Vec<u8>>, Self::Error>> + Send;
}

/// 编码器对应的通知解码结果
pub type DecodeResult<C> = Result<
    Notice<<C as query::Codec>::Task, <C as query::Codec>::Output, <C as query::Codec>::Failure>,
    <C as query::Codec>::Error,
>;

/// 结果通知编码边界, 与查询使用同一任务和证据类型
pub trait Codec: query::Codec {
    /// 严格解码已收录通知
    /// # 错误
    /// 格式, 上限, 版本或内部关联非法时返回错误
    fn decode_notice(&self, bytes: &[u8]) -> DecodeResult<Self>;
}

/// 一次观察调用的有限预算; 通知和查询回复均不刷新总期限
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Budget {
    /// 从调用开始计算的总期限
    pub timeout: Duration,
    /// 未满足结果条件时两次对账之间的间隔
    pub poll_interval: Duration,
    /// 单次只读查询的期限, 包含发送条件等待
    pub query_timeout: Duration,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(10),
            poll_interval: Duration::from_secs(1),
            query_timeout: Duration::from_millis(500),
        }
    }
}

/// 观察结束原因, State 中已合并事实仍归调用方保留
#[derive(Debug, Error)]
pub enum Error<C, T, N> {
    /// 预算必须为正, 单次查询预算不能超过总期限
    #[error("观察预算无效")]
    InvalidBudget,
    /// Client 必须绑定原 Authority
    #[error("观察必须绑定原 Authority 运行实例")]
    WrongAuthority,
    /// 有限期限内尚未取得已采用的完整证据
    #[error("结果观察超时, 已知事实保留, 不表示执行失败")]
    TimedOut,
    /// 明确的查询失败, 不删除已有事实
    #[error("结果对账查询失败: {0}")]
    Query(#[source] query::ClientError<C, T>),
    /// 通知通道失效, 调用者可以保留 State 并重建显式绑定
    #[error("结果通知通道失败: {0}")]
    Notifications(#[source] N),
    /// 同一执行的事实冲突或保留上限到达
    #[error(transparent)]
    Merge(#[from] MergeError),
}
