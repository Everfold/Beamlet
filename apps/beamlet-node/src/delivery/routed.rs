//! Authority 的选人策略与定向传输组合, 传输不依赖 Registry.
use crate::registry::Registry;
use beamlet_api::{Call, DeliveryError, Pool, RoleInstance, Selection, WorkerReport};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub(crate) struct Routed<T> {
    transport: T,
    deadline: Duration,
    registry: Registry,
}
impl<T> Routed<T> {
    pub(crate) fn new(transport: T, deadline: Duration, registry: Registry) -> Self {
        Self {
            transport,
            deadline,
            registry,
        }
    }
}
impl<T: beamlet_api::Delivery> super::Delivery for Routed<T> {
    fn select(
        &self,
        pool: &Pool,
        selection: Selection,
        capability: &str,
    ) -> Result<RoleInstance, DeliveryError> {
        self.registry
            .select(pool, selection, capability, Instant::now())
            .ok_or(DeliveryError::NoReceiver)
    }
    async fn deliver(&self, call: Call) -> Result<WorkerReport, DeliveryError> {
        self.transport.deliver(call, self.deadline).await
    }
}
