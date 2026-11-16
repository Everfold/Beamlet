use anyhow::Result;
use beamlet_protocol::{Instance, Outcome, Reply as WireReply, Submission, encode};
use std::future::Future;
use uuid::Uuid;

/// 入口交出已解析的任务与路由, 不创建领域身份.
pub(crate) struct Request {
    pub submission: Submission,
    pub reply: Reply,
}
pub(crate) trait Ingress: Send {
    fn receive(&mut self) -> impl Future<Output = Result<Option<Request>>> + Send;
    fn stop(&mut self) -> impl Future<Output = Result<()>> + Send;
    fn flush(&mut self) -> impl Future<Output = Result<()>> + Send;
}

pub(crate) struct Reply(ReplyKind);
enum ReplyKind {
    Terminal,
    Nats {
        client: Box<async_nats::Client>,
        subject: async_nats::Subject,
        request_id: Uuid,
        authority: Instance,
    },
    #[cfg(test)]
    Channel(tokio::sync::oneshot::Sender<String>),
}
impl Reply {
    pub(super) fn terminal() -> Self {
        Self(ReplyKind::Terminal)
    }
    pub(super) fn nats(
        client: async_nats::Client,
        subject: async_nats::Subject,
        request_id: Uuid,
        authority: Instance,
    ) -> Self {
        Self(ReplyKind::Nats {
            client: Box::new(client),
            subject,
            request_id,
            authority,
        })
    }
    #[cfg(test)]
    pub(super) fn channel(sender: tokio::sync::oneshot::Sender<String>) -> Self {
        Self(ReplyKind::Channel(sender))
    }

    pub(crate) async fn send(self, outcome: Outcome) -> Result<()> {
        match self.0 {
            ReplyKind::Terminal => {
                tracing::info!(event = "response.local", response = ?outcome, "本地结果")
            }
            ReplyKind::Nats {
                client,
                subject,
                request_id,
                authority,
            } => {
                client
                    .publish(
                        subject,
                        encode(&WireReply {
                            request_id,
                            authority,
                            outcome,
                        })?
                        .into(),
                    )
                    .await?;
                tracing::info!(event = "response.sent", %request_id, "已回复远端请求");
            }
            #[cfg(test)]
            ReplyKind::Channel(sender) => {
                let text = match outcome {
                    Outcome::Completed { result, .. } => result,
                    Outcome::Unresolved { reason, .. } => {
                        format!("错误: {reason}; 操作保持等待报告, 未重试")
                    }
                    Outcome::Rejected { reason } => format!("拒绝: {reason}"),
                    Outcome::Workers { .. } => anyhow::bail!("测试入口不接受节点查询"),
                };
                sender
                    .send(text)
                    .map_err(|_| anyhow::anyhow!("请求回复端已关闭"))?;
            }
        }
        Ok(())
    }
}
