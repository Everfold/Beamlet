use beamlet_primitives::{Attempt, Report};

#[derive(Debug, PartialEq, Eq)]
struct Identity(String);

#[derive(Debug, PartialEq, Eq)]
struct Output(Vec<u8>);

/// Worker 无需 Operation 或 Clone, 即可还原收到的授权并编码完整报告.
#[test]
fn worker_round_trips_owned_report_fields_without_kernel() {
    let attempt = Attempt::from_parts(Identity("operation-1".into()), Identity("attempt-1".into()));
    let report = Report::new(Identity("report-1".into()), attempt, Output(vec![1, 2, 3]));

    let (report_id, attempt, output) = report.into_parts();
    let (operation_id, attempt_id) = attempt.into_parts();
    assert_eq!(operation_id.0, "operation-1");
    assert_eq!(attempt_id.0, "attempt-1");

    let decoded = Report::new(
        report_id,
        Attempt::from_parts(operation_id, attempt_id),
        output,
    );

    assert_eq!(decoded.id().0, "report-1");
    assert_eq!(decoded.operation_id().0, "operation-1");
    assert_eq!(decoded.attempt_id().0, "attempt-1");
    assert_eq!(decoded.into_result().0, vec![1, 2, 3]);
}
