//! Console 入口: 发现角色, 选择目标并提交任务.
/// Console 的选择状态, 输入解析, 在途动作与结果展示.
mod session;
/// 应用 RON 配置的首次创建, 加载与语义校验, 不迁移已有文件.
mod settings;

use settings::{Color, Settings};
use std::process::ExitCode;
use tracing::{Instrument, error, info_span};

/// 加载 Console 配置, 初始化日志并运行交互会话.
///
/// # 返回值
///
/// 正常结束返回 `ExitCode::SUCCESS`; 配置或运行失败时记录错误并返回 `ExitCode::FAILURE`.
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
