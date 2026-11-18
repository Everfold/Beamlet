//! Node 装配角色与基础设施, 委托 Host 监督; 角色保留独立状态与身份.
use crate::{infrastructure::Infrastructure, role::RunningRole, settings::Settings};
use anyhow::{Context, Result};
use beamlet_host::Host;
use std::{collections::BTreeMap, future::Future, time::Duration};
use tokio::time::timeout;
use tracing::{Instrument, info, warn};

/// 装配基础设施和角色, 委托 Host 监督并在退出时刷新连接.
///
/// # 参数
///
/// - `settings`: 调用方提供的配置; 本函数按其身份, 期限和基础设施设置运行.
/// - `signal`: 调用方提供的退出 future; 完成表示请求停止, 错误也会触发停止并被返回.
///
/// # 返回值
///
/// 正常停止返回 `Ok(())`; 配置, 连接, 停止信号或角色监督失败时返回错误.
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
            kind = role.settings.kind.name(), label = role.settings.label.as_deref(), infrastructures = ?role.settings.infrastructures, "已分配角色会话");
    }
    tokio::pin!(signal);
    let infrastructure = async {
        let mut connected = BTreeMap::new();
        for (name, config) in settings.infrastructures {
            let span = tracing::info_span!("infrastructure", name = %name);
            let connection = Infrastructure::connect(config).instrument(span).await?;
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
    for role in roles {
        let connections = role
            .settings
            .infrastructures
            .iter()
            .map(|name| {
                infrastructures
                    .get(name)
                    .cloned()
                    .context("已校验的基础设施缺失")
            })
            .collect::<Result<Vec<_>>>()?;
        host.add(
            role.instance.role_id.to_string(),
            role.run(connections, shutdown.clone()).in_current_span(),
        );
    }
    let result = host.run(signal).await.map_err(anyhow::Error::from);
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
