//! 测试用选择表; 实际收发使用公开的进程内适配器.
use super::{Call, DeliveryError, Incoming, WorkerReport};
use anyhow::Result;
use beamlet_api::{Pool, RoleInstance, Selection, WorkerBinding, WorkerInbox};
use beamlet_transport_local::Local;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

#[derive(Clone)]
struct Worker {
    binding: WorkerBinding,
    transport: Local,
    accepting: Arc<AtomicBool>,
}
#[derive(Clone)]
pub(crate) struct Delivery {
    workers: Vec<Worker>,
    deadline: Duration,
}
impl Delivery {
    pub(crate) fn new(deadline: Duration) -> Self {
        Self {
            workers: Vec::new(),
            deadline,
        }
    }
    pub(crate) fn register(&mut self, capabilities: &[&str], capacity: usize) -> Inbox {
        let base = 3 + 2 * self.workers.len() as u128;
        self.register_in(
            Pool::default(),
            RoleInstance {
                role_id: uuid::Uuid::from_u128(base),
                session_id: uuid::Uuid::from_u128(base + 1),
            },
            capabilities,
            capacity,
        )
    }
    pub(super) fn register_in(
        &mut self,
        pool: Pool,
        instance: RoleInstance,
        capabilities: &[&str],
        capacity: usize,
    ) -> Inbox {
        let binding = WorkerBinding {
            pool,
            instance,
            capabilities: capabilities.iter().map(|name| (*name).to_owned()).collect(),
        };
        let transport = Local::new(capacity).unwrap();
        let (inner, _) = transport.bind_worker(binding.clone()).unwrap();
        let accepting = Arc::new(AtomicBool::new(true));
        self.workers.push(Worker {
            binding,
            transport,
            accepting: accepting.clone(),
        });
        Inbox { inner, accepting }
    }
}
impl super::Delivery for Delivery {
    fn select(
        &self,
        pool: &Pool,
        selection: Selection,
        capability: &str,
    ) -> Result<RoleInstance, DeliveryError> {
        self.workers
            .iter()
            .find(|worker| {
                &worker.binding.pool == pool
                    && worker.accepting.load(Ordering::Relaxed)
                    && worker.binding.capabilities.contains(capability)
                    && match selection {
                        Selection::Any => true,
                        Selection::Exact(instance) => worker.binding.instance == instance,
                    }
            })
            .map(|worker| worker.binding.instance)
            .ok_or(DeliveryError::NoReceiver)
    }
    async fn deliver(&self, call: Call) -> Result<WorkerReport, DeliveryError> {
        beamlet_api::validation::check_call(&call)?;
        let target = self.select(&call.pool, Selection::Exact(call.worker), &call.capability)?;
        let transport = &self
            .workers
            .iter()
            .find(|worker| worker.binding.pool == call.pool && worker.binding.instance == target)
            .ok_or(DeliveryError::NoReceiver)?
            .transport;
        beamlet_api::Delivery::deliver(transport, call, self.deadline).await
    }
}
pub(crate) struct Inbox {
    inner: beamlet_transport_local::WorkerEndpoint,
    accepting: Arc<AtomicBool>,
}
impl Drop for Inbox {
    fn drop(&mut self) {
        self.accepting.store(false, Ordering::Relaxed);
    }
}
impl super::Inbox for Inbox {
    async fn receive(&mut self) -> Result<Option<Incoming>> {
        Ok(self.inner.receive().await?)
    }
    async fn stop(&mut self) -> Result<()> {
        self.accepting.store(false, Ordering::Relaxed);
        Ok(self.inner.stop().await?)
    }
}
