//! 控制消息独立于 Authority 在途任务名额; 接收队列有界.
use super::{Ingress, Reply, Request};
use crate::{registry::Registry, text_reverse};
use anyhow::{Context, Result};
use beamlet_api::{
    AuthorityEvent, AuthorityInbox, AuthorityInfo, Command, DiscoveryReply, Outcome, Pool,
    Selection, Submission, Task,
};
use std::time::Instant;
use tokio::{sync::mpsc, task::JoinHandle};
use tracing::{Instrument, info, warn};

pub(crate) struct Inputs {
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
        endpoint: impl AuthorityInbox + 'static,
        info: AuthorityInfo,
        registry: Registry,
        local: crate::terminal::Input,
        input_capacity: usize,
    ) -> Result<Self> {
        endpoint.probe().await?;
        let (send, remote) = mpsc::channel(input_capacity);
        let control =
            tokio::spawn(control(endpoint, info.clone(), registry.clone(), send).in_current_span());
        let pool = if info.pools.contains(&Pool::default()) {
            Pool::default()
        } else {
            info.pools.first().context("Authority 未配置 pool")?.clone()
        };
        Ok(Self {
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
                        return Ok(Some(Request { submission: Submission { pool: self.pool.clone(), worker: Selection::Any, task: Task { capability: self.capability.clone(), payload: text } }, reply: Reply::new(|outcome| async move {
                            tracing::info!(event = "response.local", response = ?outcome, "本地结果");
                            Ok(())
                        }) }));
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
        Ok(())
    }
}
async fn control(
    mut endpoint: impl AuthorityInbox,
    info: AuthorityInfo,
    registry: Registry,
    send: mpsc::Sender<Request>,
) -> Result<()> {
    let mut cleanup = tokio::time::interval(std::time::Duration::from_secs(1));
    cleanup.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        let event = tokio::select! {
            _ = cleanup.tick() => { registry.expire(Instant::now()); continue; }
            event = endpoint.receive() => event?.context("Authority 订阅已关闭")?,
        };
        match event {
            AuthorityEvent::Connection(link) => {
                if link.connected {
                    endpoint.probe().await?;
                }
            }
            AuthorityEvent::Presence { presence, epoch } => {
                registry.observe(presence, epoch, Instant::now());
            }
            AuthorityEvent::Discovery {
                request_id, reply, ..
            } => {
                reply
                    .send(DiscoveryReply {
                        request_id,
                        authority: info.clone(),
                    })
                    .await?;
            }
            AuthorityEvent::Request {
                pool,
                command,
                reply,
            } => {
                let request_id = command.request_id();
                let authority = info.instance;
                let reply = Reply::new(move |outcome| async move {
                    reply
                        .send(beamlet_api::Reply {
                            request_id,
                            authority,
                            outcome,
                        })
                        .await
                });
                if command.authority() != info.instance
                    || command.pool() != &pool
                    || request_id.is_nil()
                {
                    reply
                        .send(Outcome::Rejected {
                            reason: "Authority 会话或 pool 不匹配".into(),
                        })
                        .await?;
                    continue;
                }
                match command {
                    Command::Workers { capability, .. } => {
                        reply
                            .send(Outcome::Workers {
                                workers: registry.workers(
                                    &pool,
                                    capability.as_deref(),
                                    Instant::now(),
                                ),
                            })
                            .await?;
                    }
                    Command::Submit { submission, .. } => {
                        if let Err(error) = send.try_send(Request { submission, reply }) {
                            error
                                .into_inner()
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
