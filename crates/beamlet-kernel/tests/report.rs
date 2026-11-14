use beamlet_kernel::{Operation, Report};

#[derive(Debug)]
struct ReportId(String);

#[derive(Debug)]
struct Output(String);

/// Worker 读取请求并生成报告, 报告可独立于 Authority 持有的 Operation 保存.
#[test]
fn report_preserves_attempt_identity_and_owns_result() {
    let (operation, attempt) = Operation::new(String::from("operation-1"), String::from("request"))
        .create_attempt(String::from("attempt-1"));
    let output = Output(operation.request().to_uppercase());
    let report = Report::new(ReportId(String::from("report-1")), attempt, output);
    assert_eq!(operation.pending_attempt_id(), "attempt-1");
    let operation = operation.accept_report(&report).expect("当前报告应被接受");
    drop(operation.close());

    assert_eq!(report.id().0, "report-1");
    assert_eq!(report.operation_id(), "operation-1");
    assert_eq!(report.attempt_id(), "attempt-1");
    assert_eq!(report.result().0, "REQUEST");
    assert_eq!(report.into_result().0, "REQUEST");
}
