//! Authority 的只读查询契约, 查询关联 ID 不属于执行身份

use crate::{
    Authority, OperationObservation, ReportObservation,
    id::{OperationId, ReportId, RoleInstanceId, SubmissionKey},
    state::OperationPhase,
};
use std::error::Error as StdError;
use thiserror::Error;

/// 调用方为每次查询提供的独立关联 ID, 在同一回复通道及其重建后保持唯一
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Id(u128);

impl Id {
    /// 返回原关联值, 编码格式由 Adapter 决定
    pub fn as_u128(self) -> u128 {
        self.0
    }
}

impl From<u128> for Id {
    fn from(value: u128) -> Self {
        Self(value)
    }
}

/// 查询选择, 不包含可用于创建执行的任务定义
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Selection {
    /// 按原提交键找回执行
    Submission(SubmissionKey),
    /// 查询请求的当前观察
    Operation(OperationId),
    /// 查询完整已保留证据, 包括附加 Report
    Report(ReportId),
}

/// 定向到明确运行实例的一次查询, 不自动解析或切换 Authority
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Request {
    /// 本次查询的关联 ID, 不是提交键
    pub id: Id,
    /// 应持有原账本的 Authority 运行实例
    pub authority_instance_id: RoleInstanceId,
    /// 所查询的原身份
    pub selection: Selection,
}

/// Authority 对一次只读查询作出的回答
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Observation<P, O, E> {
    /// 原键或 Operation 查询返回当前快照
    Operation(OperationObservation<P>),
    /// Report 查询返回完整证据及原实例方向
    Report(ReportObservation<O, E>),
    /// 当前 Authority 未保留记录, 不证明重启前从未执行
    NotFound,
}

/// 带完整查询关联的回复, 不能以其他查询的快照代替
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Reply<P, O, E> {
    /// 已由回复者检查目标实例的原请求
    pub request: Request,
    /// 当前已知事实
    pub observation: Observation<P, O, E>,
}

impl<P, O, E> Reply<P, O, E> {
    /// 校验本次关联, 选择类型, 原执行绑定与证据; 不提供发送方认证
    pub fn matches(&self, request: Request) -> bool {
        if self.request != request {
            return false;
        }
        match (&self.observation, request.selection) {
            (Observation::NotFound, _) => true,
            (
                Observation::Operation(view),
                selection @ (Selection::Submission(_) | Selection::Operation(_)),
            ) => {
                view.binding.authority_instance_id == request.authority_instance_id
                    && match selection {
                        Selection::Operation(id) => id == view.binding.operation_id,
                        _ => true,
                    }
                    && valid_operation(view)
            }
            (Observation::Report(view), Selection::Report(id)) => {
                view.authority_instance_id == request.authority_instance_id
                    && view.report.report_id == id
                    && view.report.validate().is_ok()
            }
            _ => false,
        }
    }
}

pub(crate) fn valid_operation<P>(view: &OperationObservation<P>) -> bool {
    let valid_phase = match view.phase {
        OperationPhase::Accepted | OperationPhase::Dispatched => view.adopted_report.is_none(),
        OperationPhase::Reported => view.adopted_report.is_some(),
        _ => false,
    };
    valid_phase
        && matches!(
            view.wait_status,
            crate::state::WaitStatus::Waiting | crate::state::WaitStatus::TimedOut
        )
}

/// 请求目标与当前账本实例不符, 不能回答该 Authority 的 NotFound
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("查询目标与当前 Authority 运行实例不符")]
pub struct WrongAuthority;

/// 编码器对应的查询回复及解码错误
pub type DecodeResult<C> = Result<
    Reply<<C as Codec>::Task, <C as Codec>::Output, <C as Codec>::Failure>,
    <C as Codec>::Error,
>;

/// 查询编码边界, 独立于提交编码; 接口不含执行或接纳能力
pub trait Codec: Send + Sync {
    /// 查询返回的完整任务定义
    type Task: Send;
    /// 业务输出
    type Output: Send;
    /// 业务失败证据
    type Failure: Send;
    /// 严格编码错误
    type Error: StdError + Send + Sync + 'static;

    /// 编码一次只读请求
    /// # 错误
    /// 编码或字节上限不满足时返回错误
    fn encode_query(&self, request: Request) -> Result<Vec<u8>, Self::Error>;

    /// 严格解码完整查询回复
    /// # 错误
    /// 格式, 版本或内容非法时返回错误
    fn decode_query(&self, bytes: &[u8]) -> DecodeResult<Self>;
}

/// 只读查询错误, 网络失败不能解释为记录不存在
#[derive(Debug, Error)]
pub enum ClientError<C, T> {
    /// Client 尚未绑定明确的 Authority 实例
    #[error("只读查询必须显式指定 Authority 运行实例")]
    AuthorityRequired,
    /// 本地查询编码失败
    #[error("查询编码失败: {0}")]
    Encode(#[source] C),
    /// 发送或接收失败, 没有取得本次查询的确定回答
    #[error("查询传输失败: {source}")]
    Transport {
        /// 原查询关联, 可用于审阅失败
        request: Request,
        /// 底层错误
        #[source]
        source: T,
    },
    /// 总期限内没有匹配回答, 不证明记录不存在
    #[error("查询超时, 未取得目标 Authority 的匹配回答")]
    TimedOut {
        /// 原查询关联
        request: Request,
    },
    /// 只有匹配的 Authority 回答才能产生此错误
    #[error("当前 Authority 未保留所查询的记录")]
    NotFound {
        /// 原查询关联
        request: Request,
    },
}

pub(crate) fn observe<P: Clone + Eq, O: Clone + Eq, E: Clone + Eq>(
    authority: &Authority<P, O, E>,
    selection: Selection,
) -> Observation<P, O, E> {
    match selection {
        Selection::Submission(key) => authority
            .submission(key)
            .map(OperationObservation::from_view)
            .map(Observation::Operation),
        Selection::Operation(id) => authority
            .operation(id)
            .map(OperationObservation::from_view)
            .map(Observation::Operation),
        Selection::Report(id) => authority
            .report(id)
            .map(ReportObservation::from_delivery)
            .map(Observation::Report),
    }
    .unwrap_or(Observation::NotFound)
}
