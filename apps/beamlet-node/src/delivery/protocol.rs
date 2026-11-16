//! 只在通信适配器处展开原语字段, kernel 不依赖 serde.

use super::{Call, WorkerReport};
use beamlet_primitives::{Attempt, Report};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WireCall {
    pub operation_id: String,
    pub attempt_id: String,
    pub capability: String,
    pub text: String,
}

impl From<Call> for WireCall {
    fn from(call: Call) -> Self {
        let (operation_id, attempt_id) = call.attempt.into_parts();

        Self {
            operation_id,
            attempt_id,
            capability: call.capability,
            text: call.text,
        }
    }
}

impl From<WireCall> for Call {
    /// 还原传输中的授权引用, 这不是身份认证或新授权.
    fn from(wire: WireCall) -> Self {
        Self {
            attempt: Attempt::from_parts(wire.operation_id, wire.attempt_id),
            capability: wire.capability,
            text: wire.text,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WireReport {
    report_id: String,
    operation_id: String,
    attempt_id: String,
    result: String,
}

impl From<WorkerReport> for WireReport {
    fn from(report: WorkerReport) -> Self {
        let (report_id, attempt, result) = report.into_parts();
        let (operation_id, attempt_id) = attempt.into_parts();

        Self {
            report_id,
            operation_id,
            attempt_id,
            result,
        }
    }
}

impl From<WireReport> for WorkerReport {
    /// 解码不自动接受报告, 生命周期仍由 Authority 推进.
    fn from(wire: WireReport) -> Self {
        Report::new(
            wire.report_id,
            Attempt::from_parts(wire.operation_id, wire.attempt_id),
            wire.result,
        )
    }
}
