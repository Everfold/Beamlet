//! 测试用本地定向适配器, 与 NATS 共用交付及接收接口.
use super::{Call, DeliveryError, Incoming, WorkerReport, contract};
use anyhow::Result;
use beamlet_api::{Pool, RoleInstance, Selection};
use std::time::Duration;
use tokio::{
    sync::{mpsc, oneshot},
    time::timeout,
};

#[derive(Clone)]
struct Worker {
    pool: Pool,
    instance: RoleInstance,
    capabilities: Vec<String>,
    sender: mpsc::Sender<Incoming>,
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
        let (sender, receiver) = mpsc::channel(capacity);
        self.workers.push(Worker {
            pool,
            instance,
            capabilities: capabilities.iter().map(|name| (*name).to_owned()).collect(),
            sender,
        });
        Inbox(receiver)
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
                &worker.pool == pool
                    && !worker.sender.is_closed()
                    && worker.capabilities.iter().any(|name| name == capability)
                    && match selection {
                        Selection::Any => true,
                        Selection::Exact(instance) => worker.instance == instance,
                    }
            })
            .map(|worker| worker.instance)
            .ok_or(DeliveryError::NoReceiver)
    }
    async fn deliver(&self, call: Call) -> Result<WorkerReport, DeliveryError> {
        contract::check_call(&call)?;
        let target = self.select(&call.pool, Selection::Exact(call.worker), &call.capability)?;
        let sender = &self
            .workers
            .iter()
            .find(|worker| worker.pool == call.pool && worker.instance == target)
            .ok_or(DeliveryError::NoReceiver)?
            .sender;
        let operation_id = call.attempt.operation_id().clone();
        let attempt_id = call.attempt.id().clone();
        let (reply, receive) = oneshot::channel();
        sender
            .try_send(Incoming {
                call,
                reply: contract::Reply::new(move |report| async move {
                    reply.send(report).map_err(|_| {
                        beamlet_api::ReplyError::new(std::io::Error::other("报告接收端已关闭"))
                    })
                }),
            })
            .map_err(|error| DeliveryError::NotDelivered(error.to_string()))?;
        let report = timeout(self.deadline, receive)
            .await
            .map_err(|_| DeliveryError::Unknown("等待报告超时".into()))?
            .map_err(|_| DeliveryError::Unknown("接收者未返回报告".into()))?;
        contract::check_report(report, &operation_id, &attempt_id)
    }
}
pub(crate) struct Inbox(mpsc::Receiver<Incoming>);
impl super::Inbox for Inbox {
    async fn receive(&mut self) -> Result<Option<Incoming>> {
        Ok(self.0.recv().await)
    }
    async fn stop(&mut self) -> Result<()> {
        self.0.close();
        Ok(())
    }
}
