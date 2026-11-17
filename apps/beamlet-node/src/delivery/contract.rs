use anyhow::Result;
use beamlet_api::{Call, DeliveryError, Incoming, Pool, RoleInstance, Selection, WorkerReport};
use std::future::Future;

/// 角色侧选人及执行编排, 消息后端只实现 beamlet_api::Delivery.
pub(crate) trait Delivery: Clone + Send + Sync + 'static {
    fn select(
        &self,
        pool: &Pool,
        selection: Selection,
        capability: &str,
    ) -> Result<RoleInstance, DeliveryError>;
    fn deliver(
        &self,
        call: Call,
    ) -> impl Future<Output = Result<WorkerReport, DeliveryError>> + Send;
}

pub(crate) trait Inbox: Send {
    fn receive(&mut self) -> impl Future<Output = Result<Option<Incoming>>> + Send;
    fn stop(&mut self) -> impl Future<Output = Result<()>> + Send;
}
