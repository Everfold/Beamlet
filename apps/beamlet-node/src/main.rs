mod authority;
mod capabilities;
mod delivery;
mod ingress;
mod node;
mod presence;
mod registry;
mod role;
mod settings;
mod shutdown;
mod terminal;
mod text_reverse;
mod worker;

use clap::Parser;
use settings::{Color, Settings};
use std::{path::PathBuf, process::ExitCode};
use tracing::{Instrument, error, info_span};
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
        node_session_id = %session_id,
        label = settings.label.as_deref(),
    );

    async {
        match node::run(settings, tokio::signal::ctrl_c()).await {
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
