//! Node 装配角色与传输, 委托 Host 监督; 角色保留独立状态与身份.
use crate::{role::RunningRole, settings::Settings, transport::Transport};
use anyhow::{Context, Result};
use beamlet_host::Host;
use std::{collections::BTreeMap, future::Future, time::Duration};
use tokio::time::timeout;
use tracing::{Instrument, info, warn};

/// 装配传输和角色, 委托 Host 监督并在退出时刷新连接.
///
/// # 参数
///
/// - `settings`: 加载时已校验的应用配置.
/// - `signal`: 调用方提供的退出 future; 完成表示请求停止, 错误也会触发停止并被返回.
///
/// # 返回值
///
/// 正常停止返回 `Ok(())`; 配置, 连接, 停止信号或角色监督失败时返回错误.
pub(crate) async fn run(
    settings: Settings,
    signal: impl Future<Output = std::io::Result<()>>,
) -> Result<()> {
    info!(
        event = "node.starting",
        roles = settings.roles.len(),
        transports = settings.transports.len(),
        "Node 启动"
    );
    let mut host = Host::new(beamlet_host::Options {
        shutdown_timeout: Duration::from_secs(settings.node.shutdown_timeout_secs),
    })?;
    let roles: Vec<_> = settings.roles.into_iter().map(RunningRole::new).collect();
    for role in &roles {
        info!(event = "role.prepared", role_id = %role.instance.role_id, role_session_id = %role.instance.session_id,
            kind = role.settings.name(), label = role.settings.label(), transports = ?role.settings.transports(), "已分配角色会话");
    }
    tokio::pin!(signal);
    let connect_transports = async {
        let mut connected = BTreeMap::new();
        for (name, config) in settings.transports {
            let span = tracing::info_span!("transport", name = %name);
            let connection = Transport::connect(config).instrument(span).await?;
            info!(event = "transport.ready", %name, "传输已就绪");
            connected.insert(name, connection);
        }
        Ok::<_, anyhow::Error>(connected)
    };
    let transports = tokio::select! {
        result = &mut signal => { result?; return Ok(()); }
        result = connect_transports => result?,
    };
    let shutdown = host.shutdown();
    for role in roles {
        let connections = role
            .settings
            .transports()
            .iter()
            .map(|name| transports.get(name).cloned().context("已校验的传输缺失"))
            .collect::<Result<Vec<_>>>()?;
        host.add(
            role.instance.role_id.to_string(),
            role.run(connections, shutdown.clone()).in_current_span(),
        );
    }
    let result = host.run(signal).await.map_err(anyhow::Error::from);
    for (name, transport) in &transports {
        match timeout(Duration::from_secs(1), transport.flush()).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                warn!(event = "transport.flush_failed", %name, %error, "退出时刷新失败")
            }
            Err(error) => {
                warn!(event = "transport.flush_failed", %name, %error, "退出时刷新超时")
            }
        }
    }
    info!(event = "node.stopped", "Node 已停止运行");
    result
}
