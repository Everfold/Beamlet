use crate::{
    LocalInbox, Request, RunError,
    engine::{Ingress, Reply},
    registry::Registry,
};
use beamlet_api::{
    AuthorityEvent, AuthorityInbox, AuthorityInfo, Command, DiscoveryReply, Outcome,
};
use std::time::Instant;
use tokio::{sync::mpsc, task::JoinHandle};
use tracing::Instrument;

pub(crate) struct Inputs<P, R, L> {
    remote: mpsc::Receiver<Request<P, R>>,
    local: L,
    local_closed: bool,
    stopped: bool,
    control: JoinHandle<Result<(), RunError>>,
}
impl<P: Send + 'static, R: Send + 'static, L: LocalInbox<P, R>> Inputs<P, R, L> {
    pub(crate) async fn connect(
        endpoint: impl AuthorityInbox<P, R> + 'static,
        info: AuthorityInfo,
        registry: Registry,
        local: L,
        capacity: usize,
    ) -> Result<Self, RunError> {
        endpoint.probe().await?;
        let (send, remote) = mpsc::channel(capacity);
        let control = tokio::spawn(control(endpoint, info, registry, send).in_current_span());
        Ok(Self {
            remote,
            local,
            local_closed: false,
            stopped: false,
            control,
        })
    }
}
impl<P, R, L> Drop for Inputs<P, R, L> {
    fn drop(&mut self) {
        self.control.abort();
    }
}
impl<P: Send + 'static, R: Send + 'static, L: LocalInbox<P, R>> Ingress<P, R> for Inputs<P, R, L> {
    async fn receive(&mut self) -> Result<Option<Request<P, R>>, RunError> {
        while !self.stopped {
            tokio::select! {
                request = self.remote.recv() => {
                    if let Some(request) = request { return Ok(Some(request)); }
                    (&mut self.control).await.map_err(|error| RunError::Task(beamlet_api::TransportError::new(error)))??;
                    return Err(RunError::Invariant("Authority 控制任务意外结束"));
                }
                request = self.local.receive(), if !self.local_closed => {
                    match request? { Some(request) => return Ok(Some(request)), None => self.local_closed = true }
                }
            }
        }
        Ok(None)
    }
    async fn stop(&mut self) -> Result<(), RunError> {
        self.stopped = true;
        self.control.abort();
        self.remote.close();
        while self.remote.try_recv().is_ok() {}
        self.local.stop().await?;
        Ok(())
    }
}
async fn control<P: Send + 'static, R: Send + 'static>(
    mut endpoint: impl AuthorityInbox<P, R>,
    info: AuthorityInfo,
    registry: Registry,
    send: mpsc::Sender<Request<P, R>>,
) -> Result<(), RunError> {
    let mut cleanup = tokio::time::interval(std::time::Duration::from_secs(1));
    cleanup.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        let event = tokio::select! {
            _ = cleanup.tick() => { registry.expire(Instant::now()); continue; }
            event = endpoint.receive() => event?.ok_or(RunError::Invariant("Authority 订阅已关闭"))?,
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
                    || !info.pools.contains(&pool)
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
