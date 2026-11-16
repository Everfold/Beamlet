//! 运行时入口: 合并本地终端与 NATS 请求, 不另建后台请求队列.

use super::{Ingress, Reply, Request};
use crate::delivery::MAX_IN_FLIGHT;
use anyhow::{Context, Result};
use futures::{StreamExt, future::BoxFuture};
use std::io::BufRead;
use tokio::sync::mpsc;

pub(crate) struct Inputs {
    client: async_nats::Client,
    subscription: async_nats::Subscriber,
    local: mpsc::Receiver<std::io::Result<String>>,
    local_closed: bool,
    stopped: bool,
    rejection: Option<BoxFuture<'static, Result<()>>>,
}

impl Inputs {
    /// 订阅在返回前刷新, Authority 就绪后即可接收外部请求.
    pub(crate) async fn connect(client: async_nats::Client) -> Result<Self> {
        Self::with_input(client, terminal_input()).await
    }

    /// 测试可注入终端输入, 无需占用测试进程的 stdin.
    pub(super) async fn with_input(
        client: async_nats::Client,
        local: mpsc::Receiver<std::io::Result<String>>,
    ) -> Result<Self> {
        let subscription = client
            .queue_subscribe("beamlet.reverse", "beamlet-nodes".into())
            .await?;
        client.flush().await?;

        Ok(Self {
            client,
            subscription,
            local,
            local_closed: false,
            stopped: false,
            rejection: None,
        })
    }
}

impl Ingress for Inputs {
    async fn receive(&mut self) -> Result<Option<Request>> {
        while !self.stopped {
            tokio::select! {
                result = async {
                    self.rejection.as_mut().expect("仅在存在拒绝回复时轮询").await
                }, if self.rejection.is_some() => {
                    self.rejection = None;
                    result?;
                }

                line = self.local.recv(), if !self.local_closed => {
                    match line {
                        Some(line) => return Ok(Some(Request {
                            text: line.context("读取本地输入失败")?,
                            reply: Reply::terminal(),
                        })),
                        None => self.local_closed = true,
                    }
                }

                message = self.subscription.next(), if self.rejection.is_none() => {
                    let message = message.context("NATS 订阅已关闭")?;
                    let Some(subject) = message.reply else {
                        eprintln!("[Authority节点] 忽略没有回复地址的消息");
                        continue;
                    };

                    match String::from_utf8(message.payload.to_vec()) {
                        Ok(text) => return Ok(Some(Request {
                            text,
                            reply: Reply::nats(self.client.clone(), subject),
                        })),
                        Err(_) => {
                            let client = self.client.clone();
                            // 外层 select 取消 receive 时保留发送进度, 不丢弃或重建拒绝回复.
                            self.rejection = Some(Box::pin(async move {
                                client.publish(subject, "错误: 消息必须是 UTF-8 文本".into()).await?;
                                Ok(())
                            }));
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    async fn stop(&mut self) -> Result<()> {
        if !self.stopped {
            self.subscription.unsubscribe().await?;
            self.stopped = true;
            self.local.close();

            // 与未签发的缓冲请求一样, 停止时不承诺完成输入拒绝回复.
            self.rejection = None;
        }

        Ok(())
    }

    async fn flush(&mut self) -> Result<()> {
        self.client.flush().await?;
        Ok(())
    }
}

/// 阻塞读取独立于 Tokio; 本地 EOF 只关闭本地入口, 不结束 NATS 服务.
fn terminal_input() -> mpsc::Receiver<std::io::Result<String>> {
    let (send, recv) = mpsc::channel(MAX_IN_FLIGHT);

    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            if send.blocking_send(line).is_err() {
                break;
            }
        }
    });

    recv
}
