//! Authority/传输测试的模拟执行, 真实 Worker 循环由 beamlet-worker 测试.
use crate::{
    capabilities,
    delivery::{Call, WorkerReport},
};
use anyhow::Result;
use beamlet_primitives::Report;
use tracing::info;

pub(crate) async fn report(call: Call, report_id: String) -> Result<WorkerReport> {
    let result = capabilities::execute(call.capability, call.payload).await?;

    let report = Report::new(report_id, call.attempt, result);
    info!(
        event = "report.created",
        operation_id = %report.operation_id(),
        attempt_id = %report.attempt_id(),
        report_id = %report.id(),
        "已创建 Report"
    );
    Ok(report)
}
