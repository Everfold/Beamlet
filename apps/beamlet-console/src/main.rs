use anyhow::{Context, Result};
use std::{env, io::BufRead, time::Duration};
use tokio::sync::mpsc;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let url = env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".into());

    let client = async_nats::ConnectOptions::new()
        .request_timeout(Some(Duration::from_secs(5)))
        .connect(url)
        .await
        .context("连接 NATS 失败")?;

    let mut input = terminal_input();

    let signal = tokio::signal::ctrl_c();

    tokio::pin!(signal);

    println!("[Console] 已连接");

    loop {
        let line = tokio::select! {
            result = &mut signal => { result?; break; }
            line = input.recv() => match line {
                Some(line) => line.context("读取输入失败")?,
                None => break,
            }
        };

        let response = tokio::select! {
            result = &mut signal => { result?; break; }
            response = client.request("beamlet.reverse", line.into()) => response,
        };

        match response {
            Ok(message) => println!(
                "[Console] 返回: {}",
                std::str::from_utf8(&message.payload).context("回复不是 UTF-8 文本")?
            ),

            Err(error) => eprintln!("[Console] 请求失败, 请检查节点是否在线: {error}"),
        }
    }

    client.flush().await?;
    println!("[Console] 已退出");

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
