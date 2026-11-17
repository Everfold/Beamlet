use beamlet_api::{Call, WorkerReport};
use beamlet_primitives::Report;
pub(crate) mod text_reverse {
    pub const CAPABILITY: &str = "test.reverse/v1";
}
pub(crate) async fn report(call: Call, report_id: String) -> Result<WorkerReport, crate::RunError> {
    Ok(Report::new(
        report_id,
        call.attempt,
        call.payload.chars().rev().collect(),
    ))
}
