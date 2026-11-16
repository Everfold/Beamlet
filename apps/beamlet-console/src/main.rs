mod settings;

use anyhow::{Context, Result};
use settings::Settings;
use std::{
    io::{BufRead, IsTerminal},
    process::ExitCode,
    time::Duration,
};
use tokio::sync::mpsc;
use tracing::{Instrument, error, info, info_span, warn};

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let ansi = std::io::stderr().is_terminal()
        && std::env::var_os("NO_COLOR").is_none_or(|value| value.is_empty());
    tracing_subscriber::fmt()
        .compact()
        .with_env_filter("beamlet_console=info")
        .with_target(false)
        .with_ansi(ansi)
        .with_writer(std::io::stderr)
        .init();

    async {
        match run().await {
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

async fn run() -> Result<()> {
    let settings = Settings::load()?;
    info!(event = "console.starting", "Console 启动");

    let client = async_nats::ConnectOptions::new()
        .request_timeout(Some(Duration::from_secs(5)))
        .connect(settings.nats_url)
        .await
        .context("连接 NATS 失败")?;

    let mut input = terminal_input();

    let signal = tokio::signal::ctrl_c();

    tokio::pin!(signal);

    info!(event = "console.ready", "Console 已连接; 输入文本后回车");

    loop {
        let line = tokio::select! {
            result = &mut signal => { result?; break; }
            line = input.recv() => match line {
                Some(line) => line.context("读取输入失败")?,
                None => break,
            }
        };

        info!(event = "request.send_started", "开始发送请求");
        let response = tokio::select! {
            result = &mut signal => { result?; break; }
            response = client.request("beamlet.reverse", line.into()) => response,
        };

        match response {
            Ok(message) => {
                let response =
                    std::str::from_utf8(&message.payload).context("回复不是 UTF-8 文本")?;
                info!(event = "response.received", response = ?response, "收到回复");
            }
            Err(error) => {
                warn!(event = "request.failed", error = %error, "请求失败, 请检查节点是否在线")
            }
        }
    }

    client.flush().await?;
    info!(event = "console.stopped", "Console 已退出");

    Ok(())
}

/// 阻塞终端读取独立于 Tokio, Ctrl+C 不必等待下一次回车.
fn terminal_input() -> mpsc::Receiver<std::io::Result<String>> {
    let (send, recv) = mpsc::channel(16);

    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            if send.blocking_send(line).is_err() {
                break;
            }
        }
    });

    recv
}
