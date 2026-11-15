//! 测试用本地适配器, 与 NATS 共用交付及接收接口.
use super::{Call, DeliveryError, Incoming, WorkerReport, contract};
use anyhow::Result;
use std::time::Duration;
use tokio::{
    sync::{mpsc, oneshot},
    time::timeout,
};

#[derive(Clone)]
pub(super) struct Delivery {
    workers: Vec<(Vec<String>, mpsc::Sender<Incoming>)>,
    deadline: Duration,
}

impl Delivery {
    /// 本地登记仅服务当前测试实例, 不创建全局节点表.
    pub(super) fn new(deadline: Duration) -> Self {
        Self {
            workers: Vec::new(),
            deadline,
        }
    }

    /// 登记一组真实能力和一个有界接收端, 一份调用只入一个队列.
    pub(super) fn register(&mut self, capabilities: &[&str], capacity: usize) -> Inbox {
        let (sender, receiver) = mpsc::channel(capacity);
        self.workers.push((
            capabilities.iter().map(|name| (*name).to_owned()).collect(),
            sender,
        ));
        Inbox(receiver)
    }
}

impl super::Delivery for Delivery {
    /// 选定后不回退到第二个 Worker; 队列满时明确拒绝, 入队后失联则未知.
    async fn deliver(&self, call: Call) -> Result<WorkerReport, DeliveryError> {
        contract::check_call(&call)?;
        let sender = self
            .workers
            .iter()
            .find(|(capabilities, sender)| {
                capabilities.contains(&call.capability) && !sender.is_closed()
            })
            .map(|(_, sender)| sender)
            .ok_or(DeliveryError::NoReceiver)?;
        let operation_id = call.attempt.operation_id().clone();
        let attempt_id = call.attempt.id().clone();
        let (reply, receive) = oneshot::channel();
        sender
            .try_send(Incoming {
                call,
                reply: contract::Reply::channel(reply),
            })
            .map_err(|error| DeliveryError::NotDelivered(error.to_string()))?;
        let report = timeout(self.deadline, receive)
            .await
            .map_err(|_| DeliveryError::Unknown("等待报告超时".into()))?
            .map_err(|_| DeliveryError::Unknown("接收者未返回报告".into()))?;
        contract::check_report(report, &operation_id, &attempt_id)
    }
}

pub(super) struct Inbox(mpsc::Receiver<Incoming>);

impl super::Inbox for Inbox {
    /// 关闭后仍返回已入队调用, 直到队列为空.
    async fn receive(&mut self) -> Result<Option<Incoming>> {
        Ok(self.0.recv().await)
    }

    /// 拒绝后续入队, 不取消已接收工作.
    async fn stop(&mut self) -> Result<()> {
        self.0.close();
        Ok(())
    }
}
