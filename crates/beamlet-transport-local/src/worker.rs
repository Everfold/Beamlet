use crate::{Key, Local, WorkerRoute, invalid_deadline};
use beamlet_api::{
    AuthorityEvent, Connection, Delivery, DeliveryError, Incoming, Presence, PresenceChannel,
    PresenceEvent, ReplyError, Responder, TransportError, WorkerBinding, WorkerInbox, WorkerReport,
    validation,
};
use std::time::Duration;
use tokio::{
    sync::{mpsc, oneshot, watch},
    time::timeout,
};

/// 有界调用接收端, 正常 stop 后仍可排空已接收队列.
pub struct WorkerEndpoint<P = String, R = String> {
    local: Local<P, R>,
    key: Key,
    receiver: mpsc::Receiver<Incoming<P, R>>,
}
/// Worker 的完整公告发送端与合并后的探测通知入口.
pub struct PresenceEndpoint<P = String, R = String> {
    local: Local<P, R>,
    key: Key,
    probes: watch::Receiver<u64>,
}
impl<P, R> std::fmt::Debug for WorkerEndpoint<P, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkerEndpoint")
            .field("key", &self.key)
            .finish_non_exhaustive()
    }
}
impl<P, R> std::fmt::Debug for PresenceEndpoint<P, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PresenceEndpoint")
            .field("key", &self.key)
            .finish_non_exhaustive()
    }
}
pub(crate) fn bind<P: Send + 'static, R: Send + 'static>(
    local: Local<P, R>,
    binding: WorkerBinding,
) -> Result<crate::WorkerEndpoints<P, R>, TransportError> {
    binding.validate()?;
    let key = (binding.pool.clone(), binding.instance);
    let (sender, receiver) = mpsc::channel(local.capacity);
    let (probes, receive_probes) = watch::channel(0);
    {
        let mut routes = local.routes.lock().expect("local routes lock poisoned");
        if routes.workers.contains_key(&key) {
            return Err(TransportError::new("worker already bound"));
        }
        routes.workers.insert(
            key.clone(),
            WorkerRoute {
                binding,
                sender,
                probes,
            },
        );
    }
    Ok((
        WorkerEndpoint {
            local: local.clone(),
            key: key.clone(),
            receiver,
        },
        PresenceEndpoint {
            local,
            key,
            probes: receive_probes,
        },
    ))
}
impl<P, R> Drop for WorkerEndpoint<P, R> {
    fn drop(&mut self) {
        self.local
            .routes
            .lock()
            .expect("local routes lock poisoned")
            .workers
            .remove(&self.key);
    }
}
impl<P: Send + 'static, R: Send + 'static> WorkerInbox<P, R> for WorkerEndpoint<P, R> {
    async fn receive(&mut self) -> Result<Option<Incoming<P, R>>, TransportError> {
        Ok(self.receiver.recv().await)
    }
    async fn stop(&mut self) -> Result<(), TransportError> {
        self.receiver.close();
        Ok(())
    }
}
impl<P: Send + 'static, R: Send + 'static> Delivery<P, R> for Local<P, R> {
    async fn deliver(
        &self,
        call: beamlet_api::Call<P>,
        deadline: Duration,
    ) -> Result<WorkerReport<R>, DeliveryError> {
        validation::check_call(&call)?;
        if invalid_deadline(deadline) {
            return Err(DeliveryError::NotDelivered(
                "invalid report deadline".into(),
            ));
        }
        let sender = {
            let routes = self.routes.lock().expect("local routes lock poisoned");
            let route = routes
                .workers
                .get(&(call.pool.clone(), call.worker))
                .ok_or(DeliveryError::NoReceiver)?;
            if route.sender.is_closed() {
                return Err(DeliveryError::NoReceiver);
            }
            if !route.binding.accepts(&call) {
                return Err(DeliveryError::NotDelivered(
                    "目标身份、pool 或能力不匹配".into(),
                ));
            }
            route.sender.clone()
        };
        let operation_id = call.attempt.operation_id().clone();
        let attempt_id = call.attempt.id().clone();
        let (send, receive) = oneshot::channel();
        sender
            .try_send(Incoming {
                call,
                reply: Responder::new(move |report| async move {
                    send.send(report)
                        .map_err(|_| ReplyError::new("报告接收端已关闭"))
                }),
            })
            .map_err(|error| match error {
                mpsc::error::TrySendError::Closed(_) => DeliveryError::NoReceiver,
                mpsc::error::TrySendError::Full(_) => {
                    DeliveryError::NotDelivered("接收队列已满".into())
                }
            })?;
        let report = timeout(deadline, receive)
            .await
            .map_err(|_| DeliveryError::Unknown("等待报告超时".into()))?
            .map_err(|_| DeliveryError::Unknown("接收者未返回报告".into()))?;
        validation::check_report(report, &operation_id, &attempt_id)
    }
}
impl<P: Send + 'static, R: Send + 'static> PresenceChannel for PresenceEndpoint<P, R> {
    fn connection(&self) -> Connection {
        self.local.connection().current()
    }
    async fn receive(&mut self) -> Result<PresenceEvent, TransportError> {
        self.probes.changed().await.map_err(TransportError::new)?;
        self.probes.borrow_and_update();
        Ok(PresenceEvent::Probe)
    }
    async fn publish(&self, presence: &Presence) -> Result<(), TransportError> {
        if presence.pool != self.key.0 || presence.instance != self.key.1 {
            return Err(TransportError::new("presence does not match binding"));
        }
        let routes = self
            .local
            .routes
            .lock()
            .expect("local routes lock poisoned");
        // 公告是可丢失的完整快照, 不使心跳积压阻塞其他 Authority.
        for ((pool, _), sender) in &routes.authorities {
            if pool == &presence.pool {
                let _ = sender.try_send(AuthorityEvent::Presence {
                    presence: presence.clone(),
                    epoch: 1,
                });
            }
        }
        Ok(())
    }
    async fn flush(&self) -> Result<(), TransportError> {
        Ok(())
    }
}
