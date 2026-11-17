//! 所有传输共用的调用与报告校验.
use crate::{Call, CapabilityId, DeliveryError, WorkerReport};

/// 发送前验证能力标识, 空名称不表示通配.
///
/// # Errors
/// 能力名称无效时返回明确未交付.
pub fn check_call<P>(call: &Call<P>) -> Result<(), DeliveryError> {
    CapabilityId::validate(&call.capability)
        .map_err(|_| DeliveryError::NotDelivered("能力标识长度必须为 1..=256 字节".into()))
}

/// 返回关联匹配的完整报告, 不依赖当前 Worker 租约状态.
///
/// # Errors
/// 报告身份为空或调用关联不匹配时返回结果未知.
pub fn check_report<R>(
    report: WorkerReport<R>,
    operation_id: &str,
    attempt_id: &str,
) -> Result<WorkerReport<R>, DeliveryError> {
    if report.id().is_empty()
        || report.operation_id() != operation_id
        || report.attempt_id() != attempt_id
    {
        return Err(DeliveryError::Unknown(
            "报告关联不匹配或报告身份为空".into(),
        ));
    }
    Ok(report)
}
