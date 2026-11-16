mod authority;
mod delivery;
mod ingress;
mod settings;
mod text_reverse;
mod worker;

use anyhow::{Context, Result};
use settings::{Role, Settings};
use std::{io::IsTerminal, process::ExitCode};
use tracing::{Instrument, error, info, info_span};
use uuid::Uuid;

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let ansi = std::io::stderr().is_terminal()
        && std::env::var_os("NO_COLOR").is_none_or(|value| value.is_empty());
    tracing_subscriber::fmt()
        .compact()
        .with_env_filter("beamlet_node=info")
        .with_target(false)
        .with_ansi(ansi)
        .with_writer(std::io::stderr)
        .init();

    let settings = match Settings::load() {
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

    let client = async_nats::ConnectOptions::new()
        .subscription_capacity(delivery::MAX_IN_FLIGHT)
        .connect(settings.nats_url)
        .await
        .context("连接 NATS 失败")?;

    match settings.role {
        Role::Authority => {
            let input = ingress::live::Inputs::connect(client.clone()).await?;
            let sender = delivery::nats::Delivery::new(client.clone(), delivery::REPORT_TIMEOUT);

            authority::run(
                input,
                sender,
                session_id.to_string(),
                tokio::signal::ctrl_c(),
            )
            .await?;
        }

        Role::Worker => {
            let inbox =
                delivery::nats::Inbox::subscribe(client.clone(), &[text_reverse::CAPABILITY])
                    .await?;

            worker::run(inbox, session_id.to_string(), tokio::signal::ctrl_c()).await?;
        }
    }

    client.flush().await?;
    info!(event = "node.stopped", "节点已退出");

    Ok(())
}
