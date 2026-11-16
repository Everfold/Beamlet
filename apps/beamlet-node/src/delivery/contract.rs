use anyhow::Result;
use beamlet_primitives::{Attempt, Report};
use std::{fmt, future::Future};

/// 交付元数据与既有授权同行, 不额外签发授权.
#[derive(Debug)]
pub(crate) struct Call {
    pub attempt: Attempt<String, String>,
    pub capability: String,
    pub text: String,
}

/// 完整报告直接复用领域原语, 身份格式由 node 提供.
pub(crate) type WorkerReport = Report<String, String, String, String>;

/// 只描述有证据支持的交付结果, 不表示 Operation 的状态.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum DeliveryError {
    NoReceiver,
    NotDelivered(String),
    Unknown(String),
}

impl fmt::Display for DeliveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoReceiver => write!(f, "当前没有匹配能力的接收者"),
            Self::NotDelivered(reason) => write!(f, "明确未交付: {reason}"),
            Self::Unknown(reason) => write!(f, "交付结果未知: {reason}"),
        }
    }
}

impl std::error::Error for DeliveryError {}

/// 一次交付只发送一次, 返回关联匹配的 Report 或交付错误.
pub(crate) trait Delivery: Clone + Send + Sync + 'static {
    fn deliver(
        &self,
        call: Call,
    ) -> impl Future<Output = Result<WorkerReport, DeliveryError>> + Send;
}

/// Worker 接收入口; stop 停止新接收, receive 继续排空已接收队列.
pub(crate) trait Inbox: Send {
    fn receive(&mut self) -> impl Future<Output = Result<Option<Incoming>>> + Send;

    fn stop(&mut self) -> impl Future<Output = Result<()>> + Send;
}

/// 回复入口随调用传递, Worker 无需知道 Authority 的部署位置.
pub(crate) struct Incoming {
    pub call: Call,
    pub reply: Reply,
}

/// 业务侧只能回复, 不能读取或改写传输地址.
pub(crate) struct Reply(ReplyKind);

enum ReplyKind {
    Nats(super::nats::Reply),
    #[cfg(test)]
    Channel(tokio::sync::oneshot::Sender<WorkerReport>),
}

impl Reply {
    pub(super) fn nats(reply: super::nats::Reply) -> Self {
        Self(ReplyKind::Nats(reply))
    }

    #[cfg(test)]
    pub(super) fn channel(reply: tokio::sync::oneshot::Sender<WorkerReport>) -> Self {
        Self(ReplyKind::Channel(reply))
    }

    /// 将完整报告交回原接收端, 不把发送成功解释成生命周期完成.
    pub(crate) async fn send(self, report: WorkerReport) -> Result<()> {
        match self.0 {
            ReplyKind::Nats(reply) => reply.send(report).await,
            #[cfg(test)]
            ReplyKind::Channel(sender) => sender
                .send(report)
                .map_err(|_| anyhow::anyhow!("报告接收端已关闭")),
        }
    }
}

/// 即使传输回复地址正确, 仍拒绝指向其他调用的报告.
pub(super) fn check_report(
    report: WorkerReport,
    operation_id: &str,
    attempt_id: &str,
) -> Result<WorkerReport, DeliveryError> {
    if report.operation_id() != operation_id || report.attempt_id() != attempt_id {
        return Err(DeliveryError::Unknown("报告关联不匹配".into()));
    }

    Ok(report)
}

/// 空能力标识不表示通配请求, 必须在发送前拒绝.
pub(super) fn check_call(call: &Call) -> Result<(), DeliveryError> {
    if call.capability.is_empty() || call.capability.len() > 256 {
        return Err(DeliveryError::NotDelivered(
            "能力标识长度必须为 1..=256 字节".into(),
        ));
    }

    Ok(())
}
