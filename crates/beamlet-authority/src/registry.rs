//! Authority 的本地租约视图; 每个消息空间独立观测连接和心跳.
use beamlet_api::{
    ConnectionView, Pool, Presence, PresenceState, RoleInstance, Selection, WorkerView,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

/// 单次 Authority 运行中的传输来源.
#[derive(Clone)]
pub(crate) struct Source {
    /// 稳定的交付出口下标.
    pub id: usize,
    /// 此来源的连接世代与状态.
    pub connection: ConnectionView,
}

/// 一个传输对 Worker 的有效观测.
struct Lease {
    /// 观测来源及其实时连接状态.
    source: Source,
    /// 最后一次接受此来源心跳的时间.
    received: Instant,
    /// 此来源最后接受的心跳序号.
    sequence: u64,
    /// 观测时的连接世代.
    epoch: u64,
}
impl Lease {
    /// 检查此观测在 now 是否仍处于 timeout 内, 返回是否有效.
    fn fresh(&self, now: Instant, timeout: Duration) -> bool {
        let link = self.source.connection.current();
        link.connected
            && link.epoch == self.epoch
            && now.saturating_duration_since(self.received) < timeout
    }
}

/// 同一个 pool/Worker 会话的最新状态及各传输租约.
struct Entry {
    /// 序号最新的完整公告.
    presence: Presence,
    /// 最新序号首次到达的时间, 重复转送不延长租约.
    received: Instant,
    /// 各传输独立接受的心跳, 按注册顺序选择出口.
    leases: BTreeMap<usize, Lease>,
    /// 防止重复输出失联日志.
    stale_logged: bool,
}
impl Entry {
    /// 按 now 和 timeout 返回首个有效出口; 无有效租约则返回 None.
    fn route(&self, now: Instant, timeout: Duration) -> Option<usize> {
        self.leases
            .iter()
            .find(|(_, lease)| lease.fresh(now, timeout))
            .map(|(id, _)| *id)
    }
}

/// Authority 独立维护的 Worker 登记表, 克隆时共享.
#[derive(Clone)]
pub(crate) struct Registry {
    /// 按 pool 和角色会话去重的登记记录.
    entries: Arc<Mutex<BTreeMap<(Pool, RoleInstance), Entry>>>,
    /// 接受公告的 pool 集合.
    pools: BTreeSet<Pool>,
    /// 心跳租约时长.
    timeout: Duration,
}
impl Registry {
    /// 为 pools 创建空登记表, 租约长度为 timeout.
    pub(crate) fn new(pools: BTreeSet<Pool>, timeout: Duration) -> Self {
        Self {
            entries: Arc::default(),
            pools,
            timeout,
        }
    }

    /// 接受 source 在 epoch/now 收到的 presence; 返回是否更新租约.
    pub(crate) fn observe(
        &self,
        source: &Source,
        presence: Presence,
        epoch: u64,
        now: Instant,
    ) -> bool {
        let link = source.connection.current();
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
        if let Some(entry) = entries.get(&key)
            && (presence.sequence < entry.presence.sequence
                || (presence.sequence == entry.presence.sequence && presence != entry.presence)
                || entry
                    .leases
                    .get(&source.id)
                    .is_some_and(|lease| presence.sequence <= lease.sequence))
        {
            return false;
        }
        let changed = entries.get(&key).is_none_or(|entry| {
            entry.presence.state != presence.state || entry.route(now, self.timeout).is_none()
        });
        if changed {
            tracing::info!(event = "worker.presence", pool = %presence.pool,
                role_id = %presence.instance.role_id, session_id = %presence.instance.session_id,
                label = presence.label.as_deref(), state = ?presence.state, "Worker 登记状态更新");
        }
        let sequence = presence.sequence;
        let entry = entries.entry(key).or_insert_with(|| Entry {
            presence: presence.clone(),
            received: now,
            leases: BTreeMap::new(),
            stale_logged: false,
        });
        if sequence > entry.presence.sequence {
            entry.received = now;
        }
        entry.presence = presence;
        entry.stale_logged = false;
        entry.leases.insert(
            source.id,
            Lease {
                source: source.clone(),
                received: entry.received,
                sequence,
                epoch,
            },
        );
        true
    }

    /// 在 now 标记失效记录并清理超过四倍租约的观测.
    pub(crate) fn expire(&self, now: Instant) {
        let retention = self.timeout.saturating_mul(4);
        self.entries.lock().expect("worker registry lock poisoned").retain(|_, entry| {
            if !entry.stale_logged && entry.route(now, self.timeout).is_none() {
                tracing::info!(event = "worker.stale", pool = %entry.presence.pool,
                    role_id = %entry.presence.instance.role_id, session_id = %entry.presence.instance.session_id,
                    "Worker 登记已失效, 等待新心跳");
                entry.stale_logged = true;
            }
            entry.leases.retain(|_, lease| now.saturating_duration_since(lease.received) < retention);
            !entry.leases.is_empty()
        });
    }

    /// 按 pool 和可选 capability 查询去重快照, 在 now 计算租约状态.
    pub(crate) fn workers(
        &self,
        pool: &Pool,
        capability: Option<&str>,
        now: Instant,
    ) -> Vec<WorkerView> {
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
                last_seen_ms: entry
                    .leases
                    .values()
                    .map(|lease| {
                        u64::try_from(now.saturating_duration_since(lease.received).as_millis())
                            .unwrap_or(u64::MAX)
                    })
                    .min()
                    .unwrap_or(u64::MAX),
                stale: entry.route(now, self.timeout).is_none(),
            })
            .collect()
    }

    /// 在 now 按 pool, selection 和 capability 选定会话与出口; 无匹配则返回 None.
    pub(crate) fn select(
        &self,
        pool: &Pool,
        selection: Selection,
        capability: &str,
        now: Instant,
    ) -> Option<(RoleInstance, usize)> {
        self.entries
            .lock()
            .expect("worker registry lock poisoned")
            .values()
            .filter(|entry| {
                &entry.presence.pool == pool
                    && entry.presence.capabilities.contains(capability)
                    && entry.presence.state == PresenceState::Ready
                    && match selection {
                        Selection::Any => true,
                        Selection::Exact(instance) => instance == entry.presence.instance,
                    }
            })
            .find_map(|entry| {
                entry
                    .route(now, self.timeout)
                    .map(|route| (entry.presence.instance, route))
            })
    }
}

/// 租约与连接隔离测试.
#[cfg(test)]
#[path = "../tests/unit/registry.rs"]
mod tests;
