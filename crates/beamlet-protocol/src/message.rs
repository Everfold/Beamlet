use crate::Pool;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use uuid::Uuid;

/// 持久节点身份与本次运行会话, 不根据 UUID 排序判断新旧.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Instance {
    /// 持久节点 UUID.
    pub node_id: Uuid,
    /// 本次运行的 UUID.
    pub session_id: Uuid,
}

/// Console 可选择的 Authority 实例.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityInfo {
    /// 实例身份.
    pub instance: Instance,
    /// 人类可读名称.
    pub label: Option<String>,
    /// Authority 已加入的 pool 集合.
    pub pools: BTreeSet<Pool>,
}

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

/// Worker 自报的接收状态; 失联由观察者另外计算.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresenceState {
    /// 可以尝试提交, 不表示预留了执行槽位.
    Ready,
    /// 已停止新接收, 正在排空工作.
    Draining,
    /// 本会话已正常退出.
    Offline,
}

/// Worker 完整状态快照, 第一份心跳也可直接用于登记.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Presence {
    /// Worker 所属的唯一 pool.
    pub pool: Pool,
    /// Worker 实例.
    pub instance: Instance,
    /// 显示名称.
    pub label: Option<String>,
    /// 本次运行实际启用的能力.
    pub capabilities: BTreeSet<String>,
    /// 会话内递增序号, 重复公告不续期.
    pub sequence: u64,
    /// 本次公告状态.
    pub state: PresenceState,
}

/// Authority 观察到的 Worker 状态.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerView {
    /// 最近的完整快照.
    pub presence: Presence,
    /// 距最近有效公告的毫秒数, 使用 Authority 本地时间计算.
    pub last_seen_ms: u64,
    /// 当前是否超过期限或尚未在重连后确认.
    pub stale: bool,
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

/// 只选择一次; Exact 失败不回退到其他实例.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Selection {
    /// 从指定 pool 的有效候选中选择一个.
    Any,
    /// 锁定节点与会话.
    Exact(Instance),
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
        request_id: Uuid,
        authority: Instance,
        pool: Pool,
        capability: Option<String>,
    },
    /// 请求 Authority 创建并执行 Operation.
    Submit {
        request_id: Uuid,
        authority: Instance,
        submission: Submission,
    },
}

impl Command {
    /// 请求与回复关联 ID.
    pub fn request_id(&self) -> Uuid {
        match self {
            Self::Workers { request_id, .. } | Self::Submit { request_id, .. } => *request_id,
        }
    }
    /// 指定的 Authority 实例.
    pub fn authority(&self) -> Instance {
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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Outcome {
    /// 创建 Operation 前的拒绝, 或控制请求错误.
    Rejected { reason: String },
    /// Worker 查询快照.
    Workers { workers: Vec<WorkerView> },
    /// 有效 Report 已接受且 Operation 已关闭.
    Completed {
        operation_id: String,
        attempt_id: String,
        report_id: String,
        worker: Instance,
        result: String,
    },
    /// 已签发但未取得有效 Report, Operation 仍等待原授权.
    Unresolved {
        operation_id: String,
        attempt_id: String,
        worker: Instance,
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
    pub authority: Instance,
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
    pub authority: Instance,
    /// 执行方实例.
    pub worker: Instance,
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
        pool: Pool,
        worker: Instance,
        report: ReportData,
    },
    /// 执行前拒绝, 不构造假的 Report.
    Rejected {
        pool: Pool,
        worker: Instance,
        operation_id: String,
        attempt_id: String,
        reason: String,
    },
}
