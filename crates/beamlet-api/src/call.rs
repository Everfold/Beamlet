use crate::{Pool, RoleInstance};
use beamlet_primitives::{Attempt, Report};

/// 交付元数据与既有授权同行, 不额外签发授权.
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
pub type WorkerReport<R = String> = Report<String, String, String, R>;
