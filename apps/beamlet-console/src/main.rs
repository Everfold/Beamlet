mod client;
mod session;
mod settings;

use settings::{Color, Settings};
use std::process::ExitCode;
use tracing::{Instrument, error, info_span};

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let settings = Settings::load();
    let color = settings
        .as_ref()
        .map_or(Color::Auto, |settings| settings.color);
    tracing_subscriber::fmt()
        .compact()
        .with_env_filter("beamlet_console=info")
        .with_target(false)
        .with_ansi(color.enabled())
        .with_writer(std::io::stderr)
        .init();

    async {
        let result = match settings {
            Ok(settings) => session::run(settings).await,
            Err(error) => Err(error),
        };
        match result {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                error!(event = "console.failed", error = %format!("{error:#}"), "Console 异常退出");
                ExitCode::FAILURE
            }
        }
    }
    .instrument(info_span!("console"))
    .await
}
