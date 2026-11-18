use beamlet_api::{Call, WorkerReport};
use beamlet_primitives::Report;
/// 编排测试复用的倒序能力标识.
pub(crate) mod text_reverse {
    /// 编排单元测试使用的版本化倒序能力名称, 与测试处理器一致.
    pub const CAPABILITY: &str = "test.reverse/v1";
}
/// 模拟处理测试倒序能力并组装原授权对应的报告.
pub(crate) async fn report(call: Call, report_id: String) -> Result<WorkerReport, crate::RunError> {
    Ok(Report::new(
        report_id,
        call.attempt,
        call.payload.chars().rev().collect(),
    ))
}
