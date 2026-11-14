//! NATS 与本地终端共用 Authority, channel 连接内部 Worker task.
use crate::worker;
use anyhow::{Context, Result};
use beamlet_runtime::Runtime;
use futures::StreamExt;
use std::{collections::HashMap, env, io::BufRead};
use tokio::{sync::mpsc, task::JoinSet};

/// 本地输入和 NATS 回复地址属于传输信息, 不进入领域原语.
enum Destination {
    Local,
    Nats(async_nats::Subject),
}

/// 在线节点同时监听消息, 终端输入和 Worker 报告; Ctrl+C 时排空已有请求.
pub async fn run() -> Result<()> {
    let url = env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".into());
    let client = async_nats::connect(url).await.context("连接 NATS 失败")?;

    let mut subscription = client
        .queue_subscribe("beamlet.reverse", "beamlet-nodes".into())
        .await?;
    client.flush().await?;

    let (send, calls) = mpsc::channel(16);
    let (report_sender, mut reports) = mpsc::channel(16);

    // JoinSet 在 Authority 异常退出时取消 Worker, 避免遗留后台任务.
    let mut workers = JoinSet::new();
    workers.spawn(worker::run(calls, report_sender));

    let mut input = terminal_input();
    let mut input_closed = false;
    let mut stopping = false;
    let mut runtime = Runtime::new();
    let mut destinations = HashMap::new();
    let mut next_id = 0_u64;
    let signal = tokio::signal::ctrl_c();
    tokio::pin!(signal);

    loop {
        if stopping && destinations.is_empty() {
            break;
        }

        // 同时最多保留 16 份在途调用, 满载时暂停读入.
        let incoming = tokio::select! {
            result = &mut signal, if !stopping => {
                result?;
                stopping = true;
                subscription.unsubscribe().await?;
                println!("[节点] 停止接收新请求, 等待在途报告");
                None
            }

            result = workers.join_next() => {
                result.context("缺少工作任务")?.context("工作任务异常退出")??;
                anyhow::bail!("工作任务意外结束");
            }

            report = reports.recv() => {
                let report = report.context("工作任务报告通道意外关闭")?;
                runtime.accept_report(&report)?;
                runtime.close(report.operation_id())?;

                let destination = destinations.remove(report.operation_id()).context("缺少回复来源")?;
                match destination {
                    Destination::Local => println!("[节点] 本地结果: {}", report.result()),
                    Destination::Nats(subject) => {
                        client.publish(subject, report.result().clone().into()).await?;
                        println!("[节点] 已回复远端请求, 操作={}", report.operation_id());
                    }
                }
                None
            }

            line = input.recv(), if !input_closed && !stopping && destinations.len() < 16 => {
                match line {
                    Some(line) => Some((line.context("读取本地输入失败")?, Destination::Local)),
                    None => { input_closed = true; None }
                }
            }

            message = subscription.next(), if !stopping && destinations.len() < 16 => {
                let message = message.context("NATS 订阅已关闭")?;
                match message.reply {
                    Some(reply) => match String::from_utf8(message.payload.to_vec()) {
                        Ok(text) => Some((text, Destination::Nats(reply))),

                        Err(_) => {
                            client.publish(reply, "错误: 消息必须是 UTF-8 文本".into()).await?;
                            None
                        }
                    },

                    None => {
                        eprintln!("[节点] 忽略没有回复地址的消息");
                        None
                    }
                }
            }
        };

        if let Some((text, destination)) = incoming {
            next_id = next_id.checked_add(1).context("操作身份已耗尽")?;
            runtime.create_operation(next_id, text.clone())?;

            let attempt = runtime.create_attempt(&next_id, next_id)?;
            destinations.insert(next_id, destination);
            send.send((attempt, text)).await.context("工作任务已关闭")?;

            println!("[节点] 已提交倒写请求, 操作={next_id}");
        }
    }

    drop(send);
    workers
        .join_next()
        .await
        .context("缺少工作任务")?
        .context("工作任务异常退出")??;

    client.flush().await?;
    Ok(())
}

/// 用独立线程读取阻塞终端, 避免 Tokio 标准输入阻塞运行时退出.
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
