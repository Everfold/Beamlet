//! 将一种角色的配置和基础设施装配为独立运行实例.
use crate::{
    authority, delivery,
    infrastructure::Nats,
    ingress, presence,
    registry::Registry,
    settings::{Role, RoleSettings},
    shutdown::Shutdown,
    terminal, worker,
};
use anyhow::{Context, Result};
use beamlet_protocol::{AuthorityInfo, Presence, PresenceState, RoleInstance};
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
                    let registry = Registry::new(config.pools.clone(), Duration::from_secs(config.worker_timeout_secs), infrastructure.status.clone());
                    let info = AuthorityInfo { instance, label: settings.label, pools: config.pools };
                    let input = ingress::live::Inputs::connect(infrastructure.client.clone(), info, registry.clone(), infrastructure.status, local).await?;
                    let sender = delivery::nats::Delivery::new(infrastructure.client.clone(), Duration::from_secs(config.report_timeout_secs), registry);
                    authority::run(input, sender, instance, config.max_concurrent_tasks, shutdown.wait()).await
                }
                Role::Worker(config) => {
                    let (state, changes) = watch::channel(PresenceState::Ready);
                    let inbox = delivery::nats::Inbox::subscribe(infrastructure.client.clone(), config.pool.clone(), instance, config.capabilities.clone(), state.clone()).await?;
                    let snapshot = Presence { pool: config.pool, instance, label: settings.label, capabilities: config.capabilities, sequence: 0, state: PresenceState::Ready };
                    let announcements = presence::announce(infrastructure.client.clone(), snapshot, Duration::from_secs(config.heartbeat_interval_secs), changes, infrastructure.status);
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
        let (send, receive) = mpsc::channel(delivery::RECEIVE_CAPACITY);
        let target = matches!(self.settings.kind, Role::Authority(_)).then(|| terminal::Target {
            instance: self.instance,
            label: self.settings.label.clone(),
            send,
        });
        (target, receive)
    }
}
