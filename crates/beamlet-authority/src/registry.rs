//! Authority 的本地租约视图; 不参与 Operation/Attempt 生命周期.

use beamlet_api::ConnectionView;
use beamlet_api::{Pool, Presence, PresenceState, RoleInstance, Selection, WorkerView};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

struct Entry {
    presence: Presence,
    received: Instant,
    epoch: u64,
    stale_logged: bool,
}

#[derive(Clone)]
pub(crate) struct Registry {
    entries: Arc<Mutex<BTreeMap<(Pool, RoleInstance), Entry>>>,
    pools: BTreeSet<Pool>,
    timeout: Duration,
    connection: ConnectionView,
}

impl Registry {
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

#[cfg(test)]
#[path = "../tests/unit/registry.rs"]
mod tests;
