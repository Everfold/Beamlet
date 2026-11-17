use beamlet_api::{Call, WorkerReport};
use beamlet_primitives::Report;
/// 编排测试复用的倒序能力标识。
pub(crate) mod text_reverse {
    /// 编排单元测试使用的版本化倒序能力名称，与测试处理器一致。
    pub const CAPABILITY: &str = "test.reverse/v1";
}
/// 模拟处理测试倒序能力并组装原授权对应的报告。
///
/// # 参数
///
/// - `call`：已指定 pool、Authority 和 Worker 会话的调用，包含 Attempt、能力及业务输入。
/// - `report_id`：分配给测试报告的非空身份字符串。
///
/// # 返回值
///
/// `Ok(WorkerReport)`；结果为 payload 的 Unicode 标量倒序。
pub(crate) async fn report(call: Call, report_id: String) -> Result<WorkerReport, crate::RunError> {
    Ok(Report::new(
        report_id,
        call.attempt,
        call.payload.chars().rev().collect(),
    ))
}
