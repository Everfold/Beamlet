//! 只在通信边界展开原语字段, kernel 不依赖 serde.
use super::{Call, WorkerReport};
use beamlet_primitives::{Attempt, Report};
use beamlet_protocol::{ReportData, Task, WorkerCall};

impl From<Call> for WorkerCall {
    fn from(call: Call) -> Self {
        let (operation_id, attempt_id) = call.attempt.into_parts();
        Self {
            pool: call.pool,
            authority: call.authority,
            worker: call.worker,
            operation_id,
            attempt_id,
            task: Task {
                capability: call.capability,
                text: call.text,
            },
        }
    }
}
impl From<WorkerCall> for Call {
    fn from(wire: WorkerCall) -> Self {
        Self {
            pool: wire.pool,
            authority: wire.authority,
            worker: wire.worker,
            attempt: Attempt::from_parts(wire.operation_id, wire.attempt_id),
            capability: wire.task.capability,
            text: wire.task.text,
        }
    }
}
// 两个类型均来自外部 crate, 转换用命名函数表达.
pub(super) fn encode_report(report: WorkerReport) -> ReportData {
    let (report_id, attempt, result) = report.into_parts();
    let (operation_id, attempt_id) = attempt.into_parts();
    ReportData {
        report_id,
        operation_id,
        attempt_id,
        result,
    }
}
pub(super) fn decode_report(wire: ReportData) -> WorkerReport {
    Report::new(
        wire.report_id,
        Attempt::from_parts(wire.operation_id, wire.attempt_id),
        wire.result,
    )
}
