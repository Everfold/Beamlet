use crate::{Nats, subject};
use async_nats::RequestErrorKind;
use beamlet_api::{Call, Delivery, DeliveryError, WorkerReport, validation};
use beamlet_protocol::{WorkerCall, WorkerReply, decode, encode};
use std::time::Duration;
use tokio::time::timeout;

impl Delivery for Nats {
    /// 把调用编码并发往指定 Worker 的 NATS 主题, 验证返回报告.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Nats` 的共享引用.
    /// - `call`: 已指定 pool, Authority 和 Worker 会话的调用, 包含 Attempt, 能力及业务输入.
    /// - `deadline`: 等待本次回复的最长时长, 非绝对时间; 超时不取消远端执行.
    ///
    /// # 返回值
    ///
    /// 成功返回 WorkerReport; 无接收者, 未交付或交付结果未知按 DeliveryError 区分.
    async fn deliver(&self, call: Call, deadline: Duration) -> Result<WorkerReport, DeliveryError> {
        validation::check_call(&call)?;
        if deadline.is_zero() || std::time::Instant::now().checked_add(deadline).is_none() {
            return Err(DeliveryError::NotDelivered(
                "invalid report deadline".into(),
            ));
        }
        let wire = WorkerCall::from(call);
        let bytes =
            encode(&wire).map_err(|error| DeliveryError::NotDelivered(error.to_string()))?;
        let request = async_nats::Request::new()
            .payload(bytes.into())
            .timeout(Some(deadline));
        let response = timeout(
            deadline,
            self.client
                .send_request(subject::worker(&wire.pool, wire.worker), request),
        )
        .await
        .map_err(|_| DeliveryError::Unknown("等待报告超时, Worker 可能仍在执行".into()))?
        .map_err(|error| match error.kind() {
            RequestErrorKind::NoResponders => DeliveryError::NoReceiver,
            RequestErrorKind::InvalidSubject | RequestErrorKind::MaxPayloadExceeded => {
                DeliveryError::NotDelivered(error.to_string())
            }
            _ => DeliveryError::Unknown(error.to_string()),
        })?;
        let reply: WorkerReply = decode(&response.payload)
            .map_err(|error| DeliveryError::Unknown(format!("无法解析报告: {error}")))?;
        // 校验签发时的快照, 不依赖此刻的存活状态, 允许执行期间租约过期.
        match reply {
            WorkerReply::Report {
                pool,
                worker,
                report,
            } if pool == wire.pool && worker == wire.worker => {
                if report.report_id.is_empty() {
                    return Err(DeliveryError::Unknown("报告身份为空".into()));
                }
                validation::check_report(report.into(), &wire.operation_id, &wire.attempt_id)
            }
            WorkerReply::Rejected {
                pool,
                worker,
                operation_id,
                attempt_id,
                reason,
            } if pool == wire.pool
                && worker == wire.worker
                && operation_id == wire.operation_id
                && attempt_id == wire.attempt_id =>
            {
                Err(DeliveryError::NotDelivered(reason))
            }
            _ => Err(DeliveryError::Unknown(
                "报告的 pool/Worker/会话/调用关联不匹配".into(),
            )),
        }
    }
}
