//! 控制消息独立于 Authority 在途任务名额; 接收队列有界.
use super::{Ingress, Reply, Request};
use crate::{
    delivery::MAX_IN_FLIGHT, infrastructure::Connection, registry::Registry, text_reverse,
};
use anyhow::{Context, Result};
use beamlet_protocol::{
    AuthorityInfo, Command, Discovery, DiscoveryReply, Outcome, Pool, Presence, Selection,
    Submission, Task, decode, encode, subject,
};
use futures::{FutureExt, StreamExt, stream::SelectAll};
use std::{collections::BTreeMap, time::Instant};
use tokio::{
    sync::{mpsc, watch},
    task::JoinHandle,
};
use tracing::{Instrument, info, warn};

pub(crate) struct Inputs {
    client: async_nats::Client,
    remote: mpsc::Receiver<Request>,
    local: mpsc::Receiver<std::io::Result<String>>,
    local_closed: bool,
    stopped: bool,
    control: JoinHandle<Result<()>>,
    info: AuthorityInfo,
    registry: Registry,
    pool: Pool,
    capability: String,
}
impl Inputs {
    pub(crate) async fn connect(
        client: async_nats::Client,
        info: AuthorityInfo,
        registry: Registry,
        connection: watch::Receiver<Connection>,
        local: crate::terminal::Input,
    ) -> Result<Self> {
        let mut subscriptions = SelectAll::new();
        let mut routes = BTreeMap::new();
        for pool in &info.pools {
            for (subject, kind) in [
                (subject::discovery(pool), Kind::Discovery),
                (subject::presence(pool), Kind::Presence),
                (subject::authority(pool, info.instance), Kind::Request),
            ] {
                subscriptions.push(client.subscribe(subject.clone()).await?);
                routes.insert(subject, (pool.clone(), kind));
            }
        }
        client.flush().await?;
        probe(&client, &info).await?;
        let (send, remote) = mpsc::channel(MAX_IN_FLIGHT);
        let control = tokio::spawn(
            control(
                client.clone(),
                info.clone(),
                registry.clone(),
                connection,
                subscriptions,
                routes,
                send,
            )
            .in_current_span(),
        );
        let pool = if info.pools.contains(&Pool::default()) {
            Pool::default()
        } else {
            info.pools.first().context("Authority 未配置 pool")?.clone()
        };
        Ok(Self {
            client,
            remote,
            local,
            local_closed: false,
            stopped: false,
            control,
            info,
            registry,
            pool,
            capability: text_reverse::CAPABILITY.into(),
        })
    }
}
impl Drop for Inputs {
    fn drop(&mut self) {
        self.control.abort();
    }
}
impl Ingress for Inputs {
    async fn receive(&mut self) -> Result<Option<Request>> {
        while !self.stopped {
            tokio::select! {
                request = self.remote.recv() => {
                    if let Some(request) = request { return Ok(Some(request)); }
                    (&mut self.control).await.context("Authority 控制任务异常退出")??;
                    anyhow::bail!("Authority 控制任务意外结束");
                }
                line = self.local.recv(), if !self.local_closed => {
                    let Some(line) = line else { self.local_closed = true; continue; };
                    let text = line.context("读取本地输入失败")?;
                    if let Some(pool) = text.strip_prefix("/pool ") {
                        match pool.parse::<Pool>() {
                            Ok(pool) if self.info.pools.contains(&pool) => { self.pool = pool; info!(pool = %self.pool, "已选择 pool"); }
                            _ => warn!("该 pool 不属于当前 Authority"),
                        }
                    } else if let Some(capability) = text.strip_prefix("/capability ") {
                        self.capability = capability.to_owned();
                        info!(capability = %self.capability, "已选择能力");
                    } else if text == "/workers" {
                        info!(workers = ?self.registry.workers(&self.pool, None, Instant::now()), "Worker 列表");
                    } else {
                        return Ok(Some(Request { submission: Submission { pool: self.pool.clone(), worker: Selection::Any, task: Task { capability: self.capability.clone(), text } }, reply: Reply::terminal() }));
                    }
                }
            }
        }
        Ok(None)
    }
    async fn stop(&mut self) -> Result<()> {
        self.stopped = true;
        self.control.abort();
        self.remote.close();
        self.local.close();
        Ok(())
    }
    async fn flush(&mut self) -> Result<()> {
        self.client.flush().await?;
        Ok(())
    }
}
#[derive(Clone, Copy)]
enum Kind {
    Discovery,
    Presence,
    Request,
}
async fn probe(client: &async_nats::Client, info: &AuthorityInfo) -> Result<()> {
    for pool in &info.pools {
        client
            .publish(subject::probe(pool), Vec::new().into())
            .await?;
    }
    Ok(())
}
async fn control(
    client: async_nats::Client,
    info: AuthorityInfo,
    registry: Registry,
    mut connection: watch::Receiver<Connection>,
    mut subscriptions: SelectAll<async_nats::Subscriber>,
    routes: BTreeMap<String, (Pool, Kind)>,
    send: mpsc::Sender<Request>,
) -> Result<()> {
    let mut epoch = connection.borrow_and_update().epoch;
    let mut cleanup = tokio::time::interval(std::time::Duration::from_secs(1));
    cleanup.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        let message = tokio::select! {
            biased;
            changed = connection.changed() => {
                changed.context("连接状态入口已关闭")?;
                let link = *connection.borrow_and_update();
                epoch = link.epoch;
                // 不把断线之前缓冲的心跳当作恢复连接后的存活证据.
                while subscriptions.next().now_or_never().flatten().is_some() {}
                if link.connected { client.flush().await?; probe(&client, &info).await?; }
                continue;
            }
            _ = cleanup.tick() => { registry.expire(Instant::now()); continue; }
            message = subscriptions.next() => message.context("Authority 订阅已关闭")?,
        };
        let Some((pool, kind)) = routes.get(message.subject.as_str()) else {
            continue;
        };
        if !connection.borrow().connected {
            continue;
        }
        match kind {
            Kind::Presence => match decode::<Presence>(&message.payload) {
                Ok(presence) if &presence.pool == pool => {
                    registry.observe(presence, epoch, Instant::now());
                }
                _ => warn!(event = "heartbeat.rejected", %pool, "忽略无效心跳"),
            },
            Kind::Discovery => {
                let (Some(reply), Ok(request)) =
                    (message.reply, decode::<Discovery>(&message.payload))
                else {
                    continue;
                };
                client
                    .publish(
                        reply,
                        encode(&DiscoveryReply {
                            request_id: request.request_id,
                            authority: info.clone(),
                        })?
                        .into(),
                    )
                    .await?;
            }
            Kind::Request => {
                let Some(subject) = message.reply else {
                    continue;
                };
                let request: Command = match decode(&message.payload) {
                    Ok(request) => request,
                    Err(error) => {
                        warn!(event = "request.rejected", %error, "忽略无效请求");
                        continue;
                    }
                };
                let reply =
                    Reply::nats(client.clone(), subject, request.request_id(), info.instance);
                if request.authority() != info.instance
                    || request.pool() != pool
                    || request.request_id().is_nil()
                {
                    reply
                        .send(Outcome::Rejected {
                            reason: "Authority 会话或 pool 不匹配".into(),
                        })
                        .await?;
                    continue;
                }
                match request {
                    Command::Workers { capability, .. } => {
                        reply
                            .send(Outcome::Workers {
                                workers: registry.workers(
                                    pool,
                                    capability.as_deref(),
                                    Instant::now(),
                                ),
                            })
                            .await?
                    }
                    Command::Submit { submission, .. } => {
                        if let Err(error) = send.try_send(Request { submission, reply }) {
                            let request = error.into_inner();
                            request
                                .reply
                                .send(Outcome::Rejected {
                                    reason: "Authority 接收队列已满或正在停止".into(),
                                })
                                .await?;
                        }
                    }
                }
            }
        }
    }
}
