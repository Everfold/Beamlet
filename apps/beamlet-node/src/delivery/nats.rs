//! 定向交付到一个 pool 内的一个 Worker 会话; 不使用能力共享队列.
use super::{Call, DeliveryError, Incoming, WorkerReport, contract};
use crate::registry::Registry;
use anyhow::{Context, Result};
use async_nats::{Client, RequestErrorKind};
use beamlet_api::{Pool, PresenceState, RoleInstance, Selection};
use beamlet_protocol::{WorkerCall, WorkerReply, decode, encode, subject};
use futures::StreamExt;
use std::{
    collections::BTreeSet,
    time::{Duration, Instant},
};
use tokio::{sync::watch, time::timeout};
use tracing::warn;

#[derive(Clone)]
pub(crate) struct Delivery {
    client: Client,
    deadline: Duration,
    registry: Registry,
}
impl Delivery {
    pub(crate) fn new(client: Client, deadline: Duration, registry: Registry) -> Self {
        Self {
            client,
            deadline,
            registry,
        }
    }
}
impl super::Delivery for Delivery {
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
        contract::check_call(&call)?;
        let wire = WorkerCall::from(call);
        let bytes =
            encode(&wire).map_err(|error| DeliveryError::NotDelivered(error.to_string()))?;
        let request = async_nats::Request::new()
            .payload(bytes.into())
            .timeout(Some(self.deadline));
        let response = timeout(
            self.deadline,
            self.client
                .send_request(subject::worker(&wire.pool, wire.worker), request),
        )
        .await
        .map_err(|_| DeliveryError::Unknown("等待报告超时, Worker 可能仍在执行".into()))?
        .map_err(|error| match error.kind() {
            RequestErrorKind::NoResponders => DeliveryError::NoReceiver,
            RequestErrorKind::InvalidSubject | RequestErrorKind::MaxPayloadExceeded => {
                DeliveryError::NotDelivered(error.to_string())
            }
            _ => DeliveryError::Unknown(error.to_string()),
        })?;
        let reply: WorkerReply = decode(&response.payload)
            .map_err(|error| DeliveryError::Unknown(format!("无法解析报告: {error}")))?;
        // 校验签发时的快照, 不依赖此刻的存活状态, 允许执行期间租约过期.
        match reply {
            WorkerReply::Report {
                pool,
                worker,
                report,
            } if pool == wire.pool && worker == wire.worker => {
                if report.report_id.is_empty() {
                    return Err(DeliveryError::Unknown("报告身份为空".into()));
                }
                contract::check_report(report.into(), &wire.operation_id, &wire.attempt_id)
            }
            WorkerReply::Rejected {
                pool,
                worker,
                operation_id,
                attempt_id,
                reason,
            } if pool == wire.pool
                && worker == wire.worker
                && operation_id == wire.operation_id
                && attempt_id == wire.attempt_id =>
            {
                Err(DeliveryError::NotDelivered(reason))
            }
            _ => Err(DeliveryError::Unknown(
                "报告的 pool/Worker/会话/调用关联不匹配".into(),
            )),
        }
    }
}

pub(crate) struct Inbox {
    client: Client,
    pool: Pool,
    instance: RoleInstance,
    capabilities: BTreeSet<String>,
    subscription: async_nats::Subscriber,
    state: watch::Sender<PresenceState>,
}
impl Inbox {
    pub(crate) async fn subscribe(
        client: Client,
        pool: Pool,
        instance: RoleInstance,
        capabilities: BTreeSet<String>,
        state: watch::Sender<PresenceState>,
    ) -> Result<Self> {
        let subscription = client.subscribe(subject::worker(&pool, instance)).await?;
        client.flush().await?;
        Ok(Self {
            client,
            pool,
            instance,
            capabilities,
            subscription,
            state,
        })
    }
}
impl super::Inbox for Inbox {
    async fn receive(&mut self) -> Result<Option<Incoming>> {
        while let Some(message) = self.subscription.next().await {
            let Some(reply) = message.reply else {
                warn!(event = "attempt.rejected", "忽略没有回复地址的调用");
                continue;
            };
            let wire: WorkerCall = match decode(&message.payload) {
                Ok(wire) => wire,
                Err(error) => {
                    warn!(event = "attempt.rejected", %error, "忽略无效调用");
                    continue;
                }
            };
            if wire.pool != self.pool
                || wire.worker != self.instance
                || message.subject.as_str() != subject::worker(&self.pool, self.instance)
                || wire.authority.role_id.is_nil()
                || wire.authority.session_id.is_nil()
                || wire.operation_id.is_empty()
                || wire.attempt_id.is_empty()
                || !self.capabilities.contains(&wire.task.capability)
            {
                let rejection = WorkerReply::Rejected {
                    pool: self.pool.clone(),
                    worker: self.instance,
                    operation_id: wire.operation_id,
                    attempt_id: wire.attempt_id,
                    reason: "目标身份、pool 或能力不匹配".into(),
                };
                self.client
                    .publish(reply, encode(&rejection)?.into())
                    .await?;
                warn!(
                    event = "attempt.rejected",
                    "拒绝与当前 Worker 声明不匹配的调用"
                );
                continue;
            }
            return Ok(Some(Incoming {
                call: wire.into(),
                reply: responder(Reply {
                    client: self.client.clone(),
                    subject: reply,
                    pool: self.pool.clone(),
                    worker: self.instance,
                }),
            }));
        }
        Ok(None)
    }
    async fn stop(&mut self) -> Result<()> {
        self.state.send_replace(PresenceState::Draining);
        self.subscription.drain().await?;
        self.client.flush().await?;
        Ok(())
    }
}
pub(crate) struct Reply {
    client: Client,
    subject: async_nats::Subject,
    pool: Pool,
    worker: RoleInstance,
}
impl Reply {
    pub(super) async fn send(self, report: WorkerReport) -> Result<()> {
        let reply = WorkerReply::Report {
            pool: self.pool,
            worker: self.worker,
            report: report.into(),
        };
        self.client
            .publish(self.subject, encode(&reply)?.into())
            .await
            .context("发送报告失败")
    }
}

fn responder(reply: Reply) -> contract::Reply {
    contract::Reply::new(move |report| async move {
        reply
            .send(report)
            .await
            .map_err(beamlet_api::ReplyError::new)
    })
}
