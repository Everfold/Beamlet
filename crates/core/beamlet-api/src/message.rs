use crate::{Pool, RoleInstance, Selection, Uuid, WorkerView};

/// 业务请求, 是 Operation 的不可变原始请求.
///
/// # 泛型参数
///
/// - `P`: 业务输入类型, 默认 String.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task<P = String> {
    /// 带版本的能力标识.
    pub capability: String,
    /// 调用者定义的原始业务数据.
    pub payload: P,
}

/// 调用者提交创建请求, 自身不创建 Operation.
///
/// # 泛型参数
///
/// - `P`: 业务输入类型, 默认 String.
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
///
/// # 泛型参数
///
/// - `P`: 业务输入类型, 默认 String.
#[derive(Debug, Clone)]
pub enum Command<P = String> {
    /// 查询指定 pool 的 Worker, 可按能力过滤.
    Workers {
        /// 本次请求与回复的关联 UUID, 不是幂等键或 Operation 身份.
        request_id: Uuid,
        /// 明确指定或实际应答的 Authority 角色与会话身份.
        authority: RoleInstance,
        /// 本次查询或调用所属的 pool, 按精确名称隔离.
        pool: Pool,
        /// 可选的能力过滤条件; None 表示不按能力筛选.
        capability: Option<String>,
    },
    /// 请求 Authority 创建并执行 Operation.
    Submit {
        /// 本次请求与回复的关联 UUID, 不是幂等键或 Operation 身份.
        request_id: Uuid,
        /// 明确指定或实际应答的 Authority 角色与会话身份.
        authority: RoleInstance,
        /// 创建请求的 pool, Worker 选择与业务 Task; 尚未签发授权.
        submission: Submission<P>,
    },
}

impl<P> Command<P> {
    /// 读取请求与回复关联使用的 UUID.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Command<P>` 的共享引用.
    ///
    /// # 返回值
    ///
    /// 当前动作或命令已有的请求 UUID, 不生成新身份.
    pub fn request_id(&self) -> Uuid {
        match self {
            Self::Workers { request_id, .. } | Self::Submit { request_id, .. } => *request_id,
        }
    }
    /// 读取命令明确指定的 Authority 角色与会话身份.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Command<P>` 的共享引用.
    ///
    /// # 返回值
    ///
    /// 命令中保存的 RoleInstance, 不执行发现或重新选择.
    pub fn authority(&self) -> RoleInstance {
        match self {
            Self::Workers { authority, .. } | Self::Submit { authority, .. } => *authority,
        }
    }
    /// 借用命令所属的 pool.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Command<P>` 的共享引用.
    ///
    /// # 返回值
    ///
    /// 查询或提交所携带的 pool 引用.
    pub fn pool(&self) -> &Pool {
        match self {
            Self::Workers { pool, .. } => pool,
            Self::Submit { submission, .. } => &submission.pool,
        }
    }
}

/// 最终回复; 未解决结果不伪装成 Worker Report.
///
/// Unresolved 不表示远端没有执行; 只有有效 Report 才能产生 Completed.
///
/// # 泛型参数
///
/// - `R`: 业务结果类型, 默认 String.
#[derive(Debug, Clone)]
pub enum Outcome<R = String> {
    /// 创建 Operation 前的拒绝, 或控制请求错误.
    Rejected {
        /// 拒绝或无法确认执行结果的具体诊断原因.
        reason: String,
    },
    /// Worker 查询快照.
    Workers {
        /// 该 Authority 观察到的 Worker 快照, 可能包含 Draining, Offline 或过期记录.
        workers: Vec<WorkerView>,
    },
    /// 有效 Report 已接受且 Operation 已关闭.
    Completed {
        /// Authority 为本次操作分配的 Operation 身份.
        operation_id: String,
        /// Authority 已签发, 与该操作关联的 Attempt 身份.
        attempt_id: String,
        /// Worker 生成的 Report 身份, 仅有效报告被接受后用于完成回复.
        report_id: String,
        /// 已完成该授权的 Worker 角色会话, 不从当前登记表重新推断.
        worker: RoleInstance,
        /// 执行产生的业务结果, 不在协议层重新判断成功语义.
        result: R,
    },
    /// 已签发但未取得有效 Report, Operation 仍等待原授权.
    Unresolved {
        /// Authority 为本次操作分配的 Operation 身份.
        operation_id: String,
        /// Authority 已签发, 与该操作关联的 Attempt 身份.
        attempt_id: String,
        /// 签发时锁定的 Worker 会话; 未取得报告不表示它没有执行.
        worker: RoleInstance,
        /// 拒绝或无法确认执行结果的具体诊断原因.
        reason: String,
    },
}

/// Authority 对调用者的关联回复.
///
/// # 泛型参数
///
/// - `R`: 业务结果类型, 默认 String.
#[derive(Debug, Clone)]
pub struct Reply<R = String> {
    /// 原请求 ID.
    pub request_id: Uuid,
    /// 实际处理请求的 Authority.
    pub authority: RoleInstance,
    /// 查询或提交结果.
    pub outcome: Outcome<R>,
}
