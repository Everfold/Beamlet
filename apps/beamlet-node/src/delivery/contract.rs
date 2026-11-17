use anyhow::Result;
use beamlet_api::{Call, Pool, RoleInstance, Selection, WorkerReport};
use std::{fmt, future::Future};

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
    fn select(
        &self,
        pool: &Pool,
        selection: Selection,
        capability: &str,
    ) -> Result<RoleInstance, DeliveryError>;

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

pub(crate) type Reply = beamlet_api::Responder<WorkerReport>;

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
    if beamlet_api::CapabilityId::validate(&call.capability).is_err() {
        return Err(DeliveryError::NotDelivered(
            "能力标识长度必须为 1..=256 字节".into(),
        ));
    }

    Ok(())
}
