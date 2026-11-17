use crate::{Nats, subject};
use beamlet_api::{
    AuthorityEvent, AuthorityInbox, AuthorityInfo, ConnectionView, DiscoveryReply, Pool,
    ReplyError, Responder, TransportError,
};
use beamlet_protocol::{decode, encode};
use futures::{FutureExt, StreamExt, stream::SelectAll};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy)]
enum Kind {
    Discovery,
    Presence,
    Request,
}

/// Authority 的原始控制事件入口, 与业务任务准入队列分离.
#[derive(Debug)]
pub struct AuthorityEndpoint {
    nats: Nats,
    pools: BTreeSet<Pool>,
    subscriptions: SelectAll<async_nats::Subscriber>,
    routes: BTreeMap<String, (Pool, Kind)>,
    epoch: u64,
}
impl AuthorityEndpoint {
    pub(crate) async fn bind(mut nats: Nats, info: &AuthorityInfo) -> Result<Self, TransportError> {
        if info.instance.role_id.is_nil()
            || info.instance.session_id.is_nil()
            || info.pools.is_empty()
        {
            return Err(TransportError::new("invalid authority binding"));
        }
        let mut subscriptions = SelectAll::new();
        let mut routes = BTreeMap::new();
        for pool in &info.pools {
            for (subject, kind) in [
                (subject::discovery(pool), Kind::Discovery),
                (subject::presence(pool), Kind::Presence),
                (subject::authority(pool, info.instance), Kind::Request),
            ] {
                subscriptions.push(
                    nats.client
                        .subscribe(subject.clone())
                        .await
                        .map_err(TransportError::new)?,
                );
                routes.insert(subject, (pool.clone(), kind));
            }
        }
        nats.flush().await?;
        let epoch = nats.status.borrow_and_update().epoch;
        Ok(Self {
            nats,
            pools: info.pools.clone(),
            subscriptions,
            routes,
            epoch,
        })
    }
}
impl AuthorityInbox for AuthorityEndpoint {
    fn connection(&self) -> ConnectionView {
        self.nats.connection()
    }
    async fn receive(&mut self) -> Result<Option<AuthorityEvent>, TransportError> {
        loop {
            let message = tokio::select! {
                biased;
                changed = self.nats.status.changed() => {
                    changed.map_err(TransportError::new)?;
                    let link = *self.nats.status.borrow_and_update();
                    self.epoch = link.epoch;
                    // 重连前的缓冲不能为新世代续租. 事件返回前不等待任何发送.
                    while self.subscriptions.next().now_or_never().flatten().is_some() {}
                    return Ok(Some(AuthorityEvent::Connection(link)));
                }
                message = self.subscriptions.next() => match message { Some(message) => message, None => return Ok(None) },
            };
            if !self.nats.status.borrow().connected {
                continue;
            }
            let Some((pool, kind)) = self.routes.get(message.subject.as_str()) else {
                continue;
            };
            let client = self.nats.client.clone();
            match kind {
                Kind::Presence => {
                    let presence: beamlet_api::Presence = match decode(&message.payload) {
                        Ok(presence) => presence,
                        Err(error) => {
                            tracing::warn!(event = "heartbeat.rejected", %pool, %error, "忽略无效心跳");
                            continue;
                        }
                    };
                    if &presence.pool == pool {
                        return Ok(Some(AuthorityEvent::Presence {
                            presence,
                            epoch: self.epoch,
                        }));
                    }
                }
                Kind::Discovery => {
                    let (Some(subject), Ok(request)) = (
                        message.reply,
                        decode::<beamlet_protocol::Discovery>(&message.payload),
                    ) else {
                        continue;
                    };
                    return Ok(Some(AuthorityEvent::Discovery {
                        pool: pool.clone(),
                        request_id: request.request_id,
                        reply: Responder::new(move |reply: DiscoveryReply| async move {
                            let wire = beamlet_protocol::DiscoveryReply {
                                request_id: reply.request_id,
                                authority: reply.authority,
                            };
                            client
                                .publish(subject, encode(&wire).map_err(ReplyError::new)?.into())
                                .await
                                .map_err(ReplyError::new)
                        }),
                    }));
                }
                Kind::Request => {
                    let Some(subject) = message.reply else {
                        continue;
                    };
                    let command: beamlet_protocol::Command = match decode(&message.payload) {
                        Ok(command) => command,
                        Err(error) => {
                            tracing::warn!(event = "request.rejected", %error, "忽略无效请求");
                            continue;
                        }
                    };
                    return Ok(Some(AuthorityEvent::Request {
                        pool: pool.clone(),
                        command: command.into(),
                        reply: Responder::new(move |reply: beamlet_api::Reply| async move {
                            let request_id = reply.request_id;
                            client
                                .publish(
                                    subject,
                                    encode(&beamlet_protocol::Reply::from(reply))
                                        .map_err(ReplyError::new)?
                                        .into(),
                                )
                                .await
                                .map_err(ReplyError::new)?;
                            tracing::info!(event = "response.sent", %request_id, "已回复远端请求");
                            Ok(())
                        }),
                    }));
                }
            }
        }
    }
    async fn probe(&self) -> Result<(), TransportError> {
        self.nats.flush().await?;
        for pool in &self.pools {
            self.nats
                .client
                .publish(subject::probe(pool), Vec::new().into())
                .await
                .map_err(TransportError::new)?;
        }
        Ok(())
    }
}
