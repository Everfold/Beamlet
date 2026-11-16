//! Core NATS 适配器: 每个能力一个主题, 所有 Worker 使用同一队列组.

use super::{
    Call, DeliveryError, Incoming, WorkerReport, contract,
    protocol::{WireCall, WireReport},
};
use anyhow::{Context, Result};
use async_nats::{Client, RequestErrorKind};
use futures::{StreamExt, stream::SelectAll};
use std::time::Duration;
use tokio::time::timeout;
use tracing::warn;

const QUEUE: &str = "beamlet-workers-v1";

#[derive(Clone)]
pub(crate) struct Delivery {
    client: Client,
    deadline: Duration,
}

impl Delivery {
    /// 截止时间覆盖提交与等报告, 避免连接异常时无限占用在途名额.
    pub(crate) fn new(client: Client, deadline: Duration) -> Self {
        Self { client, deadline }
    }
}

impl super::Delivery for Delivery {
    /// 不重试发送; 收到超时或不可信回复时保守返回未知.
    async fn deliver(&self, call: Call) -> Result<WorkerReport, DeliveryError> {
        contract::check_call(&call)?;

        let subject = subject(&call.capability);
        let wire = WireCall::from(call);
        let bytes =
            serde_json::to_vec(&wire).map_err(|e| DeliveryError::NotDelivered(e.to_string()))?;

        let request = async_nats::Request::new()
            .payload(bytes.into())
            .timeout(Some(self.deadline));
        let response = timeout(self.deadline, self.client.send_request(subject, request))
            .await
            .map_err(|_| DeliveryError::Unknown("等待报告超时, Worker 可能仍在执行".into()))?
            .map_err(|error| match error.kind() {
                RequestErrorKind::NoResponders => DeliveryError::NoReceiver,
                RequestErrorKind::InvalidSubject | RequestErrorKind::MaxPayloadExceeded => {
                    DeliveryError::NotDelivered(error.to_string())
                }
                _ => DeliveryError::Unknown(error.to_string()),
            })?;

        let report: WireReport = serde_json::from_slice(&response.payload)
            .map_err(|e| DeliveryError::Unknown(format!("无法解析报告: {e}")))?;
        contract::check_report(report.into(), &wire.operation_id, &wire.attempt_id)
    }
}

pub(crate) struct Inbox {
    client: Client,
    capabilities: Vec<String>,
    subscriptions: SelectAll<async_nats::Subscriber>,
}

impl Inbox {
    /// 启动时声明实际支持的能力, flush 后才对外报告就绪.
    pub(crate) async fn subscribe(client: Client, capabilities: &[&str]) -> Result<Self> {
        anyhow::ensure!(!capabilities.is_empty(), "Worker 必须声明至少一种能力");

        let mut names = Vec::new();
        let mut subscriptions = SelectAll::new();
        for capability in capabilities {
            anyhow::ensure!(
                !capability.is_empty() && capability.len() <= 256,
                "能力标识长度无效"
            );

            if !names.iter().any(|name| name == capability) {
                subscriptions.push(
                    client
                        .queue_subscribe(subject(capability), QUEUE.into())
                        .await?,
                );
                names.push((*capability).to_owned());
            }
        }

        client.flush().await?;

        Ok(Self {
            client,
            capabilities: names,
            subscriptions,
        })
    }
}

impl super::Inbox for Inbox {
    /// 跳过无回复地址、格式错误及能力与主题不一致的输入, 不执行未知工作.
    async fn receive(&mut self) -> Result<Option<Incoming>> {
        while let Some(message) = self.subscriptions.next().await {
            let Some(reply) = message.reply else {
                warn!(event = "attempt.rejected", "忽略没有回复地址的调用");
                continue;
            };

            let wire: WireCall = match serde_json::from_slice(&message.payload) {
                Ok(wire) => wire,
                Err(error) => {
                    warn!(event = "attempt.rejected", error = %error, "忽略无效调用");
                    continue;
                }
            };

            if !self.capabilities.contains(&wire.capability)
                || message.subject.as_str() != subject(&wire.capability)
            {
                warn!(
                    event = "attempt.rejected",
                    operation_id = %wire.operation_id,
                    attempt_id = %wire.attempt_id,
                    capability = %wire.capability,
                    "拒绝与声明不匹配的能力"
                );
                continue;
            }

            return Ok(Some(Incoming {
                call: wire.into(),
                reply: contract::Reply::nats(Reply {
                    client: self.client.clone(),
                    subject: reply,
                }),
            }));
        }

        Ok(None)
    }

    /// 停止新订阅交付, 已缓冲的消息仍由 receive 排空.
    async fn stop(&mut self) -> Result<()> {
        for subscription in self.subscriptions.iter_mut() {
            subscription.drain().await?;
        }

        self.client.flush().await?;

        Ok(())
    }
}

pub(crate) struct Reply {
    client: Client,
    subject: async_nats::Subject,
}

impl Reply {
    /// 报告只发到当前调用的回复地址, 不广播到共享报告主题.
    pub(super) async fn send(self, report: WorkerReport) -> Result<()> {
        let bytes = serde_json::to_vec(&WireReport::from(report))?;

        self.client
            .publish(self.subject, bytes.into())
            .await
            .context("发送报告失败")
    }
}

/// 用字节的十六进制编码保持一一对应, 不让点号或通配符改变主题结构.
fn subject(capability: &str) -> String {
    let encoded: String = capability
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();

    format!("beamlet.work.v1.{encoded}")
}
