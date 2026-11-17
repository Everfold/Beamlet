use crate::{Nats, subject};
use beamlet_api::{
    Connection, Incoming, Presence, PresenceChannel, PresenceEvent, ReplyError, Responder,
    TransportError, WorkerBinding, WorkerInbox,
};
use beamlet_protocol::{WorkerCall, WorkerReply, decode, encode};
use futures::StreamExt;

/// 已绑定唯一实例的调用接收端, 不执行任务或维护心跳计时.
#[derive(Debug)]
pub struct WorkerEndpoint {
    nats: Nats,
    binding: WorkerBinding,
    subscription: async_nats::Subscriber,
}
/// 公告发送与探测/重连接收端, 不改变公告状态或序号.
#[derive(Debug)]
pub struct PresenceEndpoint {
    nats: Nats,
    pool: beamlet_api::Pool,
    instance: beamlet_api::RoleInstance,
    probes: async_nats::Subscriber,
}

pub(crate) async fn bind(
    nats: Nats,
    binding: WorkerBinding,
) -> Result<(WorkerEndpoint, PresenceEndpoint), TransportError> {
    binding.validate()?;
    let subscription = nats
        .client
        .subscribe(subject::worker(&binding.pool, binding.instance))
        .await
        .map_err(TransportError::new)?;
    let probes = nats
        .client
        .subscribe(subject::probe(&binding.pool))
        .await
        .map_err(TransportError::new)?;
    nats.flush().await?;
    let presence = PresenceEndpoint {
        nats: nats.clone(),
        pool: binding.pool.clone(),
        instance: binding.instance,
        probes,
    };
    Ok((
        WorkerEndpoint {
            nats,
            binding,
            subscription,
        },
        presence,
    ))
}
impl WorkerInbox for WorkerEndpoint {
    async fn receive(&mut self) -> Result<Option<Incoming>, TransportError> {
        while let Some(message) = self.subscription.next().await {
            let Some(subject) = message.reply else {
                continue;
            };
            let wire: WorkerCall = match decode(&message.payload) {
                Ok(wire) => wire,
                Err(error) => {
                    tracing::warn!(event = "attempt.rejected", %error, "忽略无效调用");
                    continue;
                }
            };
            let call = wire.into();
            if !self.binding.accepts(&call) {
                let rejection = WorkerReply::Rejected {
                    pool: self.binding.pool.clone(),
                    worker: self.binding.instance,
                    operation_id: call.attempt.operation_id().clone(),
                    attempt_id: call.attempt.id().clone(),
                    reason: "目标身份、pool 或能力不匹配".into(),
                };
                self.nats
                    .client
                    .publish(
                        subject,
                        encode(&rejection).map_err(TransportError::new)?.into(),
                    )
                    .await
                    .map_err(TransportError::new)?;
                continue;
            }
            let client = self.nats.client.clone();
            let pool = self.binding.pool.clone();
            let worker = self.binding.instance;
            return Ok(Some(Incoming {
                call,
                reply: Responder::new(move |report: beamlet_api::WorkerReport| async move {
                    let reply = WorkerReply::Report {
                        pool,
                        worker,
                        report: report.into(),
                    };
                    client
                        .publish(subject, encode(&reply).map_err(ReplyError::new)?.into())
                        .await
                        .map_err(ReplyError::new)
                }),
            }));
        }
        Ok(None)
    }
    async fn stop(&mut self) -> Result<(), TransportError> {
        self.subscription
            .drain()
            .await
            .map_err(TransportError::new)?;
        self.nats.flush().await
    }
}
impl PresenceChannel for PresenceEndpoint {
    fn connection(&self) -> Connection {
        *self.nats.status.borrow()
    }
    async fn receive(&mut self) -> Result<PresenceEvent, TransportError> {
        tokio::select! {
            biased;
            changed = self.nats.status.changed() => {
                changed.map_err(TransportError::new)?;
                Ok(PresenceEvent::Connection(*self.nats.status.borrow_and_update()))
            }
            probe = self.probes.next() => {
                probe.ok_or_else(|| TransportError::new("worker probe subscription closed"))?;
                Ok(PresenceEvent::Probe)
            }
        }
    }
    async fn publish(&self, presence: &Presence) -> Result<(), TransportError> {
        if presence.pool != self.pool || presence.instance != self.instance {
            return Err(TransportError::new("presence does not match binding"));
        }
        self.nats
            .client
            .publish(
                subject::presence(&self.pool),
                encode(presence).map_err(TransportError::new)?.into(),
            )
            .await
            .map_err(TransportError::new)
    }
    async fn flush(&self) -> Result<(), TransportError> {
        self.nats.flush().await
    }
}
