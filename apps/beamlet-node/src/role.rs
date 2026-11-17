//! 将一种角色的配置和基础设施装配为独立运行实例.
use crate::{
    authority, delivery, ingress, presence,
    registry::Registry,
    settings::{Role, RoleSettings},
    shutdown::Shutdown,
    terminal, worker,
};
use anyhow::{Context, Result};
use beamlet_api::{AuthorityInfo, Presence, PresenceState, RoleInstance};
use beamlet_api::{WorkerBinding, WorkerInbox};
use beamlet_transport_nats::Nats;
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tracing::{Instrument, info, info_span};
use uuid::Uuid;

pub(crate) struct RunningRole {
    pub settings: RoleSettings,
    pub instance: RoleInstance,
}
impl RunningRole {
    pub(crate) fn new(settings: RoleSettings) -> Self {
        let instance = RoleInstance {
            role_id: settings.role_id,
            session_id: Uuid::new_v4(),
        };
        Self { settings, instance }
    }
    pub(crate) async fn run(
        self,
        infrastructure: Nats,
        local: terminal::Input,
        shutdown: Shutdown,
    ) -> Result<()> {
        let Self { settings, instance } = self;
        let span = info_span!("role", role_id = %instance.role_id, role_session_id = %instance.session_id,
            kind = settings.kind.name(), label = settings.label.as_deref(), infrastructure = %settings.infrastructure);
        async move {
            info!(event = "role.starting", "角色启动");
            let result = match settings.kind {
                Role::Authority(config) => {
                    let registry = Registry::new(config.pools.clone(), Duration::from_secs(config.worker_timeout_secs), infrastructure.connection());
                    let info = AuthorityInfo { instance, label: settings.label, pools: config.pools.clone() };
                    let input = ingress::live::Inputs::connect(infrastructure.bind_authority(&info).await?, info, registry.clone(), local, config.input_capacity).await?;
                    let sender = delivery::routed::Routed::new(infrastructure.clone(), Duration::from_secs(config.report_timeout_secs), registry);
                    authority::run(input, sender, instance, config, shutdown.wait()).await?;
                    infrastructure.flush().await?;
                    Ok(())
                }
                Role::Worker(config) => {
                    let (state, changes) = watch::channel(PresenceState::Ready);
                    let (inbox, presence_channel) = infrastructure.bind_worker(WorkerBinding { pool: config.pool.clone(), instance, capabilities: config.capabilities.clone() }).await?;
                    let inbox = AnnouncingInbox { inner: inbox, state: state.clone() };
                    let snapshot = Presence { pool: config.pool, instance, label: settings.label, capabilities: config.capabilities, sequence: 0, state: PresenceState::Ready };
                    let announcements = presence::announce(presence_channel, snapshot, Duration::from_secs(config.heartbeat_interval_secs), changes);
                    tokio::pin!(announcements);
                    let work = worker::run(inbox, instance.session_id.to_string(), config.max_concurrent_tasks, shutdown.wait());
                    tokio::pin!(work);
                    tokio::select! {
                        result = &mut announcements => { result?; anyhow::bail!("Worker 心跳任务意外结束"); }
                        result = &mut work => {
                            state.send_replace(PresenceState::Offline);
                            announcements.await?;
                            result
                        }
                    }
                }
            };
            result.with_context(|| format!("角色 {} 运行失败", instance.role_id))?;
            info!(event = "role.stopped", "角色已退出");
            Ok(())
        }.instrument(span).await
    }
    pub(crate) fn local_input(&self) -> (Option<terminal::Target>, terminal::Input) {
        let (send, receive) = mpsc::channel(match &self.settings.kind {
            Role::Authority(config) => config.input_capacity,
            Role::Worker(_) => 1,
        });
        let target = matches!(self.settings.kind, Role::Authority(_)).then(|| terminal::Target {
            instance: self.instance,
            label: self.settings.label.clone(),
            send,
        });
        (target, receive)
    }
}

struct AnnouncingInbox<I> {
    inner: I,
    state: watch::Sender<PresenceState>,
}
impl<I: WorkerInbox> delivery::Inbox for AnnouncingInbox<I> {
    async fn receive(&mut self) -> Result<Option<beamlet_api::Incoming>> {
        Ok(self.inner.receive().await?)
    }
    async fn stop(&mut self) -> Result<()> {
        self.state.send_replace(PresenceState::Draining);
        Ok(self.inner.stop().await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn each_authority_local_queue_uses_its_own_configured_capacity() {
        for capacity in [1, 3, 32] {
            let role = RunningRole::new(RoleSettings {
                role_id: Uuid::new_v4(),
                label: None,
                infrastructure: "local".into(),
                kind: Role::Authority(crate::settings::Authority {
                    input_capacity: capacity,
                    ..Default::default()
                }),
            });
            let (target, mut input) = role.local_input();
            let target = target.unwrap();
            for _ in 0..capacity {
                target.send.try_send(Ok("text".into())).unwrap();
            }
            assert!(matches!(
                target.send.try_send(Ok("excess".into())),
                Err(mpsc::error::TrySendError::Full(_))
            ));
            input.recv().await.unwrap().unwrap();
            target.send.try_send(Ok("resumed".into())).unwrap();
        }
    }
}
