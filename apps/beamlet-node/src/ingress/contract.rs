use anyhow::Result;
use std::future::Future;
use tracing::info;

/// 入口只交出有效文本及其回复入口, 不创建任何领域原语.
pub(crate) struct Request {
    pub text: String,
    pub reply: Reply,
}

/// 接收可被取消后重试, 不能因此丢失有效请求或重复发送回复.
/// None 表示全部输入结束; stop 丢弃未交出的请求, 但不使已交出的 Reply 失效.
pub(crate) trait Ingress: Send {
    fn receive(&mut self) -> impl Future<Output = Result<Option<Request>>> + Send;

    fn stop(&mut self) -> impl Future<Output = Result<()>> + Send;

    /// 已签发任务的回复处理完后刷新发送缓冲, 不承诺原调用者已收到.
    fn flush(&mut self) -> impl Future<Output = Result<()>> + Send;
}

/// 一次性回复入口, 不向 Authority 暴露传输种类或地址.
pub(crate) struct Reply(ReplyKind);

enum ReplyKind {
    Terminal,
    Nats {
        client: async_nats::Client,
        subject: async_nats::Subject,
    },
    #[cfg(test)]
    Channel(tokio::sync::oneshot::Sender<String>),
}

impl Reply {
    pub(super) fn terminal() -> Self {
        Self(ReplyKind::Terminal)
    }

    pub(super) fn nats(client: async_nats::Client, subject: async_nats::Subject) -> Self {
        Self(ReplyKind::Nats { client, subject })
    }

    #[cfg(test)]
    pub(super) fn channel(sender: tokio::sync::oneshot::Sender<String>) -> Self {
        Self(ReplyKind::Channel(sender))
    }

    /// 操作身份只用于日志; 返回文本保持原协议, 不自动重发失败的回复.
    pub(crate) async fn send(self, operation_id: &str, text: String) -> Result<()> {
        match self.0 {
            ReplyKind::Terminal => {
                info!(event = "response.local", operation_id, response = ?text, "本地结果")
            }
            ReplyKind::Nats { client, subject } => {
                client.publish(subject, text.into()).await?;
                info!(event = "response.sent", operation_id, "已回复远端请求");
            }
            #[cfg(test)]
            ReplyKind::Channel(sender) => sender
                .send(text)
                .map_err(|_| anyhow::anyhow!("请求回复端已关闭"))?,
        }

        Ok(())
    }
}
