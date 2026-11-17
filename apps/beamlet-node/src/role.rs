//! 将一种角色的配置和基础设施装配为独立运行实例.
use crate::{
    local,
    settings::{Role, RoleSettings},
    terminal,
};
use anyhow::{Context, Result};
use beamlet_api::{AuthorityInbox, AuthorityInfo, RoleInstance};
use beamlet_host::Shutdown;
use beamlet_transport_nats::Nats;
use tokio::sync::mpsc;
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
                    let info = AuthorityInfo {
                        instance,
                        label: settings.label,
                        pools: config.pools.clone(),
                    };
                    let endpoint = infrastructure.bind_authority(&info).await?;
                    let authority = beamlet_authority::Authority::new(
                        info.clone(),
                        (&config).into(),
                        endpoint.connection(),
                    )?;
                    let input = local::Input::new(info, authority.directory(), local)?;
                    authority
                        .run_with_local(endpoint, infrastructure.clone(), input, shutdown.wait())
                        .await?;
                    infrastructure.flush().await?;
                    Ok(())
                }
                Role::Worker(config) => {
                    let mut builder = beamlet_worker::Worker::builder(
                        instance,
                        crate::capabilities::registered()?,
                    )
                    .pool(config.pool.clone())
                    .options((&config).into())
                    .enabled(config.capabilities);
                    if let Some(label) = settings.label {
                        builder = builder.label(label);
                    }
                    let worker = builder.build()?;
                    let (inbox, presence) = infrastructure.bind_worker(worker.binding()).await?;
                    worker
                        .run(inbox, presence, shutdown.wait())
                        .await
                        .map_err(anyhow::Error::from)
                }
            };
            result.with_context(|| format!("角色 {} 运行失败", instance.role_id))?;
            info!(event = "role.stopped", "角色已退出");
            Ok(())
        }
        .instrument(span)
        .await
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
