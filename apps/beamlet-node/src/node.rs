//! Node 装配角色与基础设施, 委托 Host 监督; 角色保留独立状态与身份.
use crate::{
    role::RunningRole,
    settings::{Infrastructure, Settings},
    terminal,
};
use anyhow::{Context, Result};
use beamlet_host::Host;
use beamlet_transport_nats::{Nats, Options};
use std::{collections::BTreeMap, future::Future, time::Duration};
use tokio::time::timeout;
use tracing::{Instrument, info, warn};

pub(crate) async fn run(
    settings: Settings,
    signal: impl Future<Output = std::io::Result<()>>,
) -> Result<()> {
    settings.validate()?;
    info!(
        event = "node.starting",
        roles = settings.roles.len(),
        infrastructures = settings.infrastructures.len(),
        "Node 启动"
    );
    let mut host = Host::new(beamlet_host::Options {
        shutdown_timeout: Duration::from_secs(settings.shutdown_timeout_secs),
    })?;
    let roles: Vec<_> = settings.roles.into_iter().map(RunningRole::new).collect();
    for role in &roles {
        info!(event = "role.prepared", role_id = %role.instance.role_id, role_session_id = %role.instance.session_id,
            kind = role.settings.kind.name(), label = role.settings.label.as_deref(), infrastructure = %role.settings.infrastructure, "已分配角色会话");
    }
    tokio::pin!(signal);
    let infrastructure = async {
        let mut connected = BTreeMap::new();
        for (name, config) in settings.infrastructures {
            let span = tracing::info_span!("infrastructure", name = %name);
            let connection = match config {
                Infrastructure::Nats {
                    url,
                    connect_timeout_secs,
                    subscription_capacity,
                } => {
                    let deadline = Duration::from_secs(connect_timeout_secs);
                    timeout(
                        deadline,
                        Nats::connect(
                            &url,
                            Options {
                                connect_timeout: deadline,
                                subscription_capacity,
                            },
                        )
                        .instrument(span),
                    )
                    .await
                    .context("连接基础设施超时")?
                    .context("连接 NATS 失败")?
                }
            };
            info!(event = "infrastructure.ready", %name, "基础设施已就绪");
            connected.insert(name, connection);
        }
        Ok::<_, anyhow::Error>(connected)
    };
    let infrastructures = tokio::select! {
        result = &mut signal => { result?; return Ok(()); }
        result = infrastructure => result?,
    };
    let shutdown = host.shutdown();
    let mut targets = Vec::new();
    for role in roles {
        let infrastructure = infrastructures
            .get(&role.settings.infrastructure)
            .context("已校验的基础设施缺失")?
            .clone();
        let (target, local) = role.local_input();
        targets.extend(target);
        host.add(
            role.instance.role_id.to_string(),
            role.run(infrastructure, local, shutdown.clone())
                .in_current_span(),
        );
    }
    let result = host
        .run_with_background(signal, terminal::run(targets, shutdown))
        .await
        .map_err(anyhow::Error::from);
    for (name, infrastructure) in &infrastructures {
        match timeout(Duration::from_secs(1), infrastructure.flush()).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                warn!(event = "infrastructure.flush_failed", %name, %error, "退出时刷新失败")
            }
            Err(error) => {
                warn!(event = "infrastructure.flush_failed", %name, %error, "退出时刷新超时")
            }
        }
    }
    info!(event = "node.stopped", "Node 已停止运行");
    result
}
