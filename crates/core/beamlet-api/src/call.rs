use crate::{Pool, RoleInstance};
use beamlet_primitives::{Attempt, Report};

/// 交付元数据与既有授权同行, 不额外签发授权.
///
/// 只携带既有授权; 接收者仍需校验目标和授权关联.
///
/// # 泛型参数
///
/// - `P`: 业务输入类型, 默认 String.
#[derive(Debug)]
pub struct Call<P = String> {
    /// 目标所在的 pool.
    pub pool: Pool,
    /// 签发授权的 Authority 实例.
    pub authority: RoleInstance,
    /// 唯一目标 Worker 实例.
    pub worker: RoleInstance,
    /// 已经签发的执行授权.
    pub attempt: Attempt<String, String>,
    /// 带版本的能力标识.
    pub capability: String,
    /// 调用者定义的业务数据.
    pub payload: P,
}

/// 完整报告直接复用领域原语, 身份格式由调用方提供.
///
/// # 泛型参数
///
/// - `R`: 业务结果类型, 默认 String.
pub type WorkerReport<R = String> = Report<String, String, String, R>;

/// Worker 对已接收调用的一次性响应, 区分完整报告与执行期失败.
///
/// 执行期失败不证明远端未产生副作用, 不可伪装为 NotDelivered.
///
/// # 泛型参数
///
/// - `R`: 业务结果类型, 默认 String.
#[derive(Debug)]
pub enum WorkerResponse<R = String> {
    /// 执行结束并生成完整 Report.
    Report(
        /// 关联原 Attempt 与业务结果的完整报告.
        WorkerReport<R>,
    ),
    /// 执行期间失败且未生成 Report, 仅携带诊断原因.
    Failed(
        /// 处理器执行失败的诊断原因.
        String,
    ),
}

impl<R> From<WorkerReport<R>> for WorkerResponse<R> {
    /// 把完整报告包装为 `WorkerResponse::Report`.
    ///
    /// # 参数
    ///
    /// - `report`: 关联原 Attempt 与业务结果的完整报告.
    ///
    /// # 返回值
    ///
    /// 包含传入报告的 `WorkerResponse::Report`.
    fn from(report: WorkerReport<R>) -> Self {
        Self::Report(report)
    }
}
