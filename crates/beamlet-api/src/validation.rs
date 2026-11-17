//! 所有传输共用的调用与报告校验.
use crate::{Call, CapabilityId, DeliveryError, WorkerReport};

/// 校验调用能力标识的非空和字节长度约束。
///
/// # 参数
///
/// - `call`：已指定 pool、Authority 和 Worker 会话的调用，包含 Attempt、能力及业务输入。
///
/// # 返回值
///
/// 合法返回 `Ok(())`；非法返回 NotDelivered。此函数不负责 Worker 目标或授权身份校验。
///
/// # Errors
/// 能力名称无效时返回明确未交付.
pub fn check_call<P>(call: &Call<P>) -> Result<(), DeliveryError> {
    CapabilityId::validate(&call.capability)
        .map_err(|_| DeliveryError::NotDelivered("能力标识长度必须为 1..=256 字节".into()))
}

/// 校验报告非空身份及 Operation、Attempt 关联。
///
/// # 参数
///
/// - `report`：完整报告，包含 Operation/Attempt 关联、报告身份及业务结果。
/// - `operation_id`：预期关联的 Operation 身份。
/// - `attempt_id`：本次执行授权的身份，由调用方分配。
///
/// # 返回值
///
/// 成功返回传入报告的所有权；关联错误返回 Unknown，不把错误报告当作完成。
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
