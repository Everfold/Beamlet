use beamlet_api::{AuthorityInfo, Pool, RoleInstance, Selection, WorkerView};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 一次发现查询; 回复可以有多份.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Discovery {
    /// 查询关联 ID, 不提供创建幂等性.
    pub request_id: Uuid,
}

/// 与一次发现查询关联的 Authority 信息.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscoveryReply {
    /// 原查询 ID.
    pub request_id: Uuid,
    /// 就绪的 Authority.
    pub authority: AuthorityInfo,
}

/// 业务请求, 是 Operation 的不可变原始请求.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    /// 带版本的能力标识.
    pub capability: String,
    /// 原始文本, 不修剪空白.
    pub text: String,
}

/// Console 提交创建请求, 自身不创建 Operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Submission {
    /// 路由分组.
    pub pool: Pool,
    /// Worker 选择约束.
    pub worker: Selection,
    /// 业务数据.
    pub task: Task,
}

/// Authority 请求入口支持的命令.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    /// 查询指定 pool 的 Worker, 可按能力过滤.
    Workers {
        /// 本次请求与回复的关联 UUID，不是幂等键或 Operation 身份。
        request_id: Uuid,
        /// 明确指定或实际应答的 Authority 角色与会话身份。
        authority: RoleInstance,
        /// 本次查询或调用所属的 pool，按精确名称隔离。
        pool: Pool,
        /// 可选的能力过滤条件；None 表示不按能力筛选。
        capability: Option<String>,
    },
    /// 请求 Authority 创建并执行 Operation.
    Submit {
        /// 本次请求与回复的关联 UUID，不是幂等键或 Operation 身份。
        request_id: Uuid,
        /// 明确指定或实际应答的 Authority 角色与会话身份。
        authority: RoleInstance,
        /// 创建请求的 pool、Worker 选择与业务 Task；尚未签发授权。
        submission: Submission,
    },
}

impl Command {
    /// 读取请求与回复关联使用的 UUID。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Command` 的共享引用。
    ///
    /// # 返回值
    ///
    /// 当前动作或命令已有的请求 UUID，不生成新身份。
    pub fn request_id(&self) -> Uuid {
        match self {
            Self::Workers { request_id, .. } | Self::Submit { request_id, .. } => *request_id,
        }
    }
    /// 读取命令明确指定的 Authority 角色与会话身份。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Command` 的共享引用。
    ///
    /// # 返回值
    ///
    /// 命令中保存的 RoleInstance，不执行发现或重新选择。
    pub fn authority(&self) -> RoleInstance {
        match self {
            Self::Workers { authority, .. } | Self::Submit { authority, .. } => *authority,
        }
    }
    /// 借用命令所属的 pool。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Command` 的共享引用。
    ///
    /// # 返回值
    ///
    /// 查询或提交所携带的 pool 引用。
    pub fn pool(&self) -> &Pool {
        match self {
            Self::Workers { pool, .. } => pool,
            Self::Submit { submission, .. } => &submission.pool,
        }
    }
}

/// 最终回复; 未解决结果不伪装成 Worker Report.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Outcome {
    /// 创建 Operation 前的拒绝, 或控制请求错误.
    Rejected {
        /// 拒绝或无法确认执行结果的具体诊断原因。
        reason: String,
    },
    /// Worker 查询快照.
    Workers {
        /// 该 Authority 观察到的 Worker 快照，可能包含 Draining、Offline 或过期记录。
        workers: Vec<WorkerView>,
    },
    /// 有效 Report 已接受且 Operation 已关闭.
    Completed {
        /// Authority 为本次操作分配的 Operation 身份。
        operation_id: String,
        /// Authority 已签发、与该操作关联的 Attempt 身份。
        attempt_id: String,
        /// Worker 生成的 Report 身份，仅有效报告被接受后用于完成回复。
        report_id: String,
        /// 指定或实际执行此调用的 Worker 角色与会话身份。
        worker: RoleInstance,
        /// 执行产生的业务结果，不在协议层重新判断成功语义。
        result: String,
    },
    /// 已签发但未取得有效 Report, Operation 仍等待原授权.
    Unresolved {
        /// Authority 为本次操作分配的 Operation 身份。
        operation_id: String,
        /// Authority 已签发、与该操作关联的 Attempt 身份。
        attempt_id: String,
        /// 指定或实际执行此调用的 Worker 角色与会话身份。
        worker: RoleInstance,
        /// 拒绝或无法确认执行结果的具体诊断原因。
        reason: String,
    },
}

/// Authority 对 Console 的关联回复.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    /// 原请求 ID.
    pub request_id: Uuid,
    /// 实际处理请求的 Authority.
    pub authority: RoleInstance,
    /// 查询或提交结果.
    pub outcome: Outcome,
}

/// Worker 定向调用封套, 不额外签发授权.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerCall {
    /// 路由 pool.
    pub pool: Pool,
    /// 签发方实例.
    pub authority: RoleInstance,
    /// 执行方实例.
    pub worker: RoleInstance,
    /// Operation 身份.
    pub operation_id: String,
    /// 已签发 Attempt 身份.
    pub attempt_id: String,
    /// 执行所需请求数据.
    pub task: Task,
}

/// 完整 Report 的线上数据.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReportData {
    /// Report 身份.
    pub report_id: String,
    /// Operation 身份.
    pub operation_id: String,
    /// Attempt 身份.
    pub attempt_id: String,
    /// 业务结果.
    pub result: String,
}

/// Worker 回复同时携带被调用实例, 不能用当前登记表代替来源关联校验.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkerReply {
    /// 执行已结束的证据.
    Report {
        /// 原 Worker 调用所属的 pool，用于回复关联校验。
        pool: Pool,
        /// 指定或实际执行此调用的 Worker 角色与会话身份。
        worker: RoleInstance,
        /// 完整报告的线格式字段，保留原授权与业务结果。
        report: ReportData,
    },
    /// 执行前拒绝, 不构造假的 Report.
    Rejected {
        /// 被拒绝调用所在的 pool。
        pool: Pool,
        /// 指定或实际执行此调用的 Worker 角色与会话身份。
        worker: RoleInstance,
        /// Authority 为本次操作分配的 Operation 身份。
        operation_id: String,
        /// Authority 已签发、与该操作关联的 Attempt 身份。
        attempt_id: String,
        /// 拒绝或无法确认执行结果的具体诊断原因。
        reason: String,
    },
}
