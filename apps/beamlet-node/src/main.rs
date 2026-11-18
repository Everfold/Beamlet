//! Node 入口: 加载配置, 装配角色并协调退出.
/// Node 内置文本处理能力的注册, 校验与阻塞计算安排.
mod capabilities;
/// Local 与 NATS 基础设施创建及角色端点绑定.
mod infrastructure;
/// 节点基础设施与角色装配, 委托 Host 监督并处理连接收尾.
mod node;
/// 单个 Authority 或 Worker 配置与运行会话的装配.
mod role;
/// 应用 RON 配置的首次创建, 加载与语义校验, 不迁移已有文件.
mod settings;
/// 内置 text.reverse/v1 能力的 Unicode 标量倒序处理.
mod text_reverse;

use clap::Parser;
use settings::{Color, Settings};
use std::{path::PathBuf, process::ExitCode};
use tracing::{Instrument, error, info_span};
use uuid::Uuid;

/// Node 命令行入口; 仅选择配置文件名, 角色与运行参数从配置文件读取.
#[derive(Parser)]
#[command(version, about = "启动 Beamlet 节点")]
struct Args {
    /// 可执行文件旁的单个配置文件名, 默认 node.ron, 不接受目录路径.
    #[arg(short, long, value_name = "FILE", default_value = "node.ron")]
    config: PathBuf,
}

/// 解析配置文件名, 初始化节点日志并启动 Node.
///
/// # 返回值
///
/// 正常结束返回 `ExitCode::SUCCESS`; 配置或运行失败返回 `ExitCode::FAILURE`.
#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let args = Args::parse();
    let settings = Settings::load(&args.config);
    let color = settings
        .as_ref()
        .map_or(Color::Auto, |settings| settings.node.color);

    tracing_subscriber::fmt()
        .compact()
        .with_env_filter("beamlet=info")
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
        node_id = %settings.node.node_id,
        node_session_id = %session_id,
        label = settings.node.label.as_deref(),
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
