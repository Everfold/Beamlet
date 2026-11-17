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
#[derive(Clone)]
pub(crate) struct Missing;
impl crate::engine::Dispatch for Missing {
    fn select(
        &self,
        _: &beamlet_api::Pool,
        _: beamlet_api::Selection,
        _: &str,
    ) -> Result<beamlet_api::RoleInstance, beamlet_api::DeliveryError> {
        Err(beamlet_api::DeliveryError::NoReceiver)
    }
    async fn deliver(&self, _: Call) -> Result<WorkerReport, beamlet_api::DeliveryError> {
        panic!("no Worker selected")
    }
}
