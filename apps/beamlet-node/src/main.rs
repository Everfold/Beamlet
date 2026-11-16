mod authority;
mod capabilities;
mod delivery;
mod ingress;
mod presence;
mod registry;
mod settings;
mod text_reverse;
mod worker;

use anyhow::{Context, Result};
use beamlet_protocol::{AuthorityInfo, Instance, Presence, PresenceState};
use clap::Parser;
use registry::{Connection, Registry};
use settings::{Color, Role, Settings};
use std::time::Duration;
use std::{path::PathBuf, process::ExitCode};
use tokio::sync::watch;
use tracing::{Instrument, error, info, info_span};
use uuid::Uuid;

#[derive(Parser)]
#[command(version, about = "启动 Beamlet 节点")]
struct Args {
    #[arg(short, long, value_name = "FILE", default_value = "node.ron")]
    config: PathBuf,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let args = Args::parse();
    let settings = Settings::load(&args.config);
    let color = settings
        .as_ref()
        .map_or(Color::Auto, |settings| settings.color);

    tracing_subscriber::fmt()
        .compact()
        .with_env_filter("beamlet_node=info")
        .with_target(false)
        .with_ansi(color.enabled())
        .with_writer(std::io::stderr)
        .init();

    let settings = match settings {
        Ok(settings) => settings,
        Err(error) => {
            error!(event = "node.failed", error = %format!("{error:#}"), "加载配置失败");
            return ExitCode::FAILURE;
        }
    };

    let session_id = Uuid::new_v4();
    let span = info_span!(
        "node",
        node_id = %settings.node_id,
        session_id = %session_id,
        role = match settings.role {
            Role::Authority => "authority",
            Role::Worker => "worker",
        },
        label = settings.label.as_deref(),
    );

    async {
        match run(settings, session_id).await {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                error!(event = "node.failed", error = %format!("{error:#}"), "节点异常退出");
                ExitCode::FAILURE
            }
        }
    }
    .instrument(span)
    .await
}

async fn run(settings: Settings, session_id: Uuid) -> Result<()> {
    info!(event = "node.starting", "节点启动");

    let instance = Instance {
        node_id: settings.node_id,
        session_id,
    };

    let (connection, status) = watch::channel(Connection {
        epoch: 0,
        connected: false,
    });

    let events = connection.clone();

    let client = async_nats::ConnectOptions::new()
        .subscription_capacity(delivery::MAX_IN_FLIGHT)
        .event_callback(move |event| {
            let events = events.clone();
            async move {
                let connected = match event {
                    async_nats::Event::Connected => Some(true),
                    async_nats::Event::Disconnected | async_nats::Event::Closed => Some(false),
                    _ => None,
                };
                if let Some(connected) = connected {
                    events.send_modify(|state| {
                        state.epoch += 1;
                        state.connected = connected;
                    });
                    info!(
                        event = "transport.connection",
                        connected, "NATS 连接状态变化"
                    );
                }
            }
        })
        .connect(settings.nats_url)
        .await
        .context("连接 NATS 失败")?;

    // connect 完成先建立初始状态; 后续事件始终推进连接世代.
    connection.send_if_modified(|state| {
        if state.epoch == 0 {
            state.epoch = 1;
            state.connected = true;
            true
        } else {
            false
        }
    });

    match settings.role {
        Role::Authority => {
            let registry = Registry::new(
                settings.pools.clone(),
                Duration::from_secs(settings.worker_timeout_secs),
                status.clone(),
            );

            let info = AuthorityInfo {
                instance,
                label: settings.label,
                pools: settings.pools,
            };

            let input =
                ingress::live::Inputs::connect(client.clone(), info, registry.clone(), status)
                    .await?;

            let sender =
                delivery::nats::Delivery::new(client.clone(), delivery::REPORT_TIMEOUT, registry);

            authority::run(input, sender, instance, tokio::signal::ctrl_c()).await?;
        }
        Role::Worker => {
            let (state, changes) = watch::channel(PresenceState::Ready);

            let inbox = delivery::nats::Inbox::subscribe(
                client.clone(),
                settings.pool.clone(),
                instance,
                settings.capabilities.clone(),
                state.clone(),
            )
            .await?;

            let snapshot = Presence {
                pool: settings.pool,
                instance,
                label: settings.label,
                capabilities: settings.capabilities,
                sequence: 0,
                state: PresenceState::Ready,
            };

            let announcements = presence::announce(
                client.clone(),
                snapshot,
                Duration::from_secs(settings.heartbeat_interval_secs),
                changes,
                status,
            );

            tokio::pin!(announcements);
            let work = worker::run(inbox, session_id.to_string(), tokio::signal::ctrl_c());
            tokio::pin!(work);
            tokio::select! {
                result = &mut announcements => { result?; anyhow::bail!("Worker 心跳任务意外结束"); }
                result = &mut work => {
                    state.send_replace(PresenceState::Offline);
                    announcements.await?;
                    result?;
                }
            }
        }
    }

    client.flush().await?;
    info!(event = "node.stopped", "节点已退出");

    Ok(())
}
