use crate::{Pool, RoleInstance, Selection, Uuid, WorkerView};

/// 业务请求, 是 Operation 的不可变原始请求.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task<P = String> {
    /// 带版本的能力标识.
    pub capability: String,
    /// 调用者定义的原始业务数据.
    pub payload: P,
}

/// 调用者提交创建请求, 自身不创建 Operation.
#[derive(Debug, Clone)]
pub struct Submission<P = String> {
    /// 路由分组.
    pub pool: Pool,
    /// Worker 选择约束.
    pub worker: Selection,
    /// 业务数据.
    pub task: Task<P>,
}

/// Authority 请求入口支持的命令.
#[derive(Debug, Clone)]
pub enum Command<P = String> {
    /// 查询指定 pool 的 Worker, 可按能力过滤.
    Workers {
        request_id: Uuid,
        authority: RoleInstance,
        pool: Pool,
        capability: Option<String>,
    },
    /// 请求 Authority 创建并执行 Operation.
    Submit {
        request_id: Uuid,
        authority: RoleInstance,
        submission: Submission<P>,
    },
}

impl<P> Command<P> {
    /// 请求与回复关联 ID.
    pub fn request_id(&self) -> Uuid {
        match self {
            Self::Workers { request_id, .. } | Self::Submit { request_id, .. } => *request_id,
        }
    }
    /// 指定的 Authority 实例.
    pub fn authority(&self) -> RoleInstance {
        match self {
            Self::Workers { authority, .. } | Self::Submit { authority, .. } => *authority,
        }
    }
    /// 请求所属的 pool.
    pub fn pool(&self) -> &Pool {
        match self {
            Self::Workers { pool, .. } => pool,
            Self::Submit { submission, .. } => &submission.pool,
        }
    }
}

/// 最终回复; 未解决结果不伪装成 Worker Report.
#[derive(Debug, Clone)]
pub enum Outcome<R = String> {
    /// 创建 Operation 前的拒绝, 或控制请求错误.
    Rejected { reason: String },
    /// Worker 查询快照.
    Workers { workers: Vec<WorkerView> },
    /// 有效 Report 已接受且 Operation 已关闭.
    Completed {
        operation_id: String,
        attempt_id: String,
        report_id: String,
        worker: RoleInstance,
        result: R,
    },
    /// 已签发但未取得有效 Report, Operation 仍等待原授权.
    Unresolved {
        operation_id: String,
        attempt_id: String,
        worker: RoleInstance,
        reason: String,
    },
}

/// Authority 对调用者的关联回复.
#[derive(Debug, Clone)]
pub struct Reply<R = String> {
    /// 原请求 ID.
    pub request_id: Uuid,
    /// 实际处理请求的 Authority.
    pub authority: RoleInstance,
    /// 查询或提交结果.
    pub outcome: Outcome<R>,
}
