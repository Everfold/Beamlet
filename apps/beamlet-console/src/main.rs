mod session;
mod settings;

use settings::{Color, Settings};
use std::process::ExitCode;
use tracing::{Instrument, error, info_span};

/// 加载 Console 配置、初始化日志并运行交互会话。
///
/// # 返回值
///
/// 正常结束返回 `ExitCode::SUCCESS`；配置或运行失败时记录错误并返回 `ExitCode::FAILURE`。
#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let settings = Settings::load();
    let color = settings
        .as_ref()
        .map_or(Color::Auto, |settings| settings.color);
    tracing_subscriber::fmt()
        .compact()
        .with_env_filter("beamlet=info")
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
