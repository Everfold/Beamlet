use crate::Pool;
use std::collections::BTreeSet;
use uuid::Uuid;

/// 持久角色身份与本次运行会话, 不根据 UUID 排序判断新旧.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct RoleInstance {
    /// 持久角色 UUID.
    pub role_id: Uuid,
    /// 本次运行的 UUID.
    pub session_id: Uuid,
}

/// Console 可选择的 Authority 实例.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct AuthorityInfo {
    /// 实例身份.
    pub instance: RoleInstance,
    /// 人类可读名称.
    pub label: Option<String>,
    /// Authority 已加入的 pool 集合.
    pub pools: BTreeSet<Pool>,
}

/// Worker 自报的接收状态; 失联由观察者另外计算.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum PresenceState {
    /// 可以尝试提交, 不表示预留了执行槽位.
    Ready,
    /// 已停止新接收, 正在排空工作.
    Draining,
    /// 本会话已正常退出.
    Offline,
}

/// Worker 完整状态快照, 第一份心跳也可直接用于登记.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct Presence {
    /// Worker 所属的唯一 pool.
    pub pool: Pool,
    /// Worker 实例.
    pub instance: RoleInstance,
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
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct WorkerView {
    /// 最近的完整快照.
    pub presence: Presence,
    /// 距最近有效公告的毫秒数, 使用 Authority 本地时间计算.
    pub last_seen_ms: u64,
    /// 当前是否超过期限或尚未在重连后确认.
    pub stale: bool,
}

/// 只选择一次; Exact 失败不回退到其他实例.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Selection {
    /// 从指定 pool 的有效候选中选择一个.
    Any,
    /// 锁定角色与会话.
    Exact(RoleInstance),
}
