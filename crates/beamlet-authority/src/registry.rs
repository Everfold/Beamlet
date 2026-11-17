//! Authority 的本地租约视图; 不参与 Operation/Attempt 生命周期.

use beamlet_api::ConnectionView;
use beamlet_api::{Pool, Presence, PresenceState, RoleInstance, Selection, WorkerView};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

/// 一个 pool/Worker 会话的最新有效公告及租约观测信息.
struct Entry {
    /// 最近一次被接受的完整 Worker 快照.
    presence: Presence,
    /// Authority 收到此有效快照的本地单调时间.
    received: Instant,
    /// 收到该公告时的连接世代, 必须与当前连接视图一致.
    epoch: u64,
    /// 是否已记录这次失联日志, 防止清理循环重复输出.
    stale_logged: bool,
}

/// Authority 独立维护的 Worker 登记表, 克隆时共享.
#[derive(Clone)]
pub(crate) struct Registry {
    /// 按 pool 和 Worker 角色会话索引的共享登记记录; 锁只用于短期同步访问.
    entries: Arc<Mutex<BTreeMap<(Pool, RoleInstance), Entry>>>,
    /// 本 Authority 允许登记和选择的 pools.
    pools: BTreeSet<Pool>,
    /// 有效快照的租约时长; 记录保留期为其四倍.
    timeout: Duration,
    /// 随时读取传输当前连接世代与状态的视图.
    connection: ConnectionView,
}

impl Registry {
    /// 创建 Authority 自己的 Worker 登记视图.
    ///
    /// # 参数
    ///
    /// - `pools`: 本 Authority 接受完整公告并提供选择的 pool 集合.
    /// - `timeout`: Worker 完整公告的有效租约时长.
    /// - `connection`: 非阻塞读取连接状态的视图, 连接世代变化使旧租约失效.
    ///
    /// # 返回值
    ///
    /// 空 Registry; 连接状态通过注入的 ConnectionView 读取.
    pub(crate) fn new(
        pools: BTreeSet<Pool>,
        timeout: Duration,
        connection: ConnectionView,
    ) -> Self {
        Self {
            entries: Arc::default(),
            pools,
            timeout,
            connection,
        }
    }

    /// 接收完整 Worker 公告, 按 pool, 连接世代及递增序号更新租约.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Registry` 的共享引用.
    /// - `presence`: Worker 的完整状态快照, 包括 pool, 角色会话, 能力, 序号和运行状态.
    /// - `epoch`: 收到公告时的连接观测世代, 必须等于当前连接世代.
    /// - `now`: 本次观测或查询使用的单调时间, 不读取远端时钟.
    ///
    /// # 返回值
    ///
    /// 快照被接受并写入时返回 `true`; 无关, 过期世代, 非法或重复/倒退序号返回 `false`.
    pub(crate) fn observe(&self, presence: Presence, epoch: u64, now: Instant) -> bool {
        let link = self.connection.current();
        if !link.connected
            || link.epoch != epoch
            || !self.pools.contains(&presence.pool)
            || presence.instance.role_id.is_nil()
            || presence.instance.session_id.is_nil()
            || presence.sequence == 0
            || presence.capabilities.is_empty()
            || presence
                .capabilities
                .iter()
                .any(|cap| beamlet_api::CapabilityId::validate(cap).is_err())
        {
            return false;
        }
        let mut entries = self.entries.lock().expect("worker registry lock poisoned");
        let key = (presence.pool.clone(), presence.instance);
        if entries
            .get(&key)
            .is_some_and(|entry| presence.sequence <= entry.presence.sequence)
        {
            return false;
        }
        let changed = entries.get(&key).is_none_or(|entry| {
            entry.presence.state != presence.state
                || entry.epoch != epoch
                || now.saturating_duration_since(entry.received) >= self.timeout
        });
        if changed {
            tracing::info!(event = "worker.presence", pool = %presence.pool,
                role_id = %presence.instance.role_id, session_id = %presence.instance.session_id,
                label = presence.label.as_deref(), state = ?presence.state, "Worker 登记状态更新");
        }
        entries.insert(
            key,
            Entry {
                presence,
                received: now,
                epoch,
                stale_logged: false,
            },
        );
        true
    }

    /// 标记租约失效, 并清理超过保留期的登记记录.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Registry` 的共享引用.
    /// - `now`: 本次观测或查询使用的单调时间, 不读取远端时钟.
    ///
    /// # 返回值
    ///
    /// `()`; 更新失联日志标记并移除超过四倍租约期限的记录.
    pub(crate) fn expire(&self, now: Instant) {
        let retention = self.timeout.saturating_mul(4);
        let link = self.connection.current();
        self.entries.lock().expect("worker registry lock poisoned").retain(|_, entry| {
            let age = now.saturating_duration_since(entry.received);
            if !entry.stale_logged && (age >= self.timeout || entry.epoch != link.epoch || !link.connected) {
                tracing::info!(event = "worker.stale", pool = %entry.presence.pool,
                    role_id = %entry.presence.instance.role_id, session_id = %entry.presence.instance.session_id,
                    "Worker 登记已失效, 等待新心跳");
                entry.stale_logged = true;
            }
            age < retention
        });
    }

    /// 查询 pool 中的 Worker 快照, 按可选能力过滤.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Registry` 的共享引用.
    /// - `pool`: 查询, 候选选择或主题构造的目标 pool.
    /// - `capability`: 可选能力过滤条件; None 返回该 pool 中所有能力的记录.
    /// - `now`: 本次观测或查询使用的单调时间, 不读取远端时钟.
    ///
    /// # 返回值
    ///
    /// 匹配记录的 Vec, 包含 Ready, Draining, Offline 和过期状态; stale 在查询时计算.
    pub(crate) fn workers(
        &self,
        pool: &Pool,
        capability: Option<&str>,
        now: Instant,
    ) -> Vec<WorkerView> {
        let link = self.connection.current();
        self.entries
            .lock()
            .expect("worker registry lock poisoned")
            .values()
            .filter(|entry| {
                &entry.presence.pool == pool
                    && capability.is_none_or(|cap| entry.presence.capabilities.contains(cap))
            })
            .map(|entry| WorkerView {
                presence: entry.presence.clone(),
                last_seen_ms: u64::try_from(
                    now.saturating_duration_since(entry.received).as_millis(),
                )
                .unwrap_or(u64::MAX),
                stale: !link.connected
                    || entry.epoch != link.epoch
                    || now.saturating_duration_since(entry.received) >= self.timeout,
            })
            .collect()
    }

    /// 从登记表选出状态, 期限, 能力及目标约束均匹配的 Worker.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Registry` 的共享引用.
    /// - `pool`: 查询, 候选选择或主题构造的目标 pool.
    /// - `selection`: Worker 选择约束: Any 选择可用候选, Exact 锁定角色与会话且不回退.
    /// - `capability`: 要调用或匹配的能力标识, 保留精确拼写.
    /// - `now`: 本次观测或查询使用的单调时间, 不读取远端时钟.
    ///
    /// # 返回值
    ///
    /// 有候选返回 `Some(RoleInstance)`, 否则返回 `None`; 不替换 Exact 会话.
    pub(crate) fn select(
        &self,
        pool: &Pool,
        selection: Selection,
        capability: &str,
        now: Instant,
    ) -> Option<RoleInstance> {
        self.workers(pool, Some(capability), now)
            .into_iter()
            .find(|worker| {
                !worker.stale
                    && worker.presence.state == PresenceState::Ready
                    && match selection {
                        Selection::Any => true,
                        Selection::Exact(instance) => instance == worker.presence.instance,
                    }
            })
            .map(|worker| worker.presence.instance)
    }
}

/// 当前模块关键行为与边界条件的单元测试, 实现在 tests/unit 目录.
#[cfg(test)]
#[path = "../tests/unit/registry.rs"]
mod tests;
