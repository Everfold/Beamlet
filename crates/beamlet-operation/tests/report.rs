use beamlet_kernel::{AcceptReportError, OperationState};
use beamlet_operation::{Operation, OperationRecord, Report};

#[derive(Debug)]
struct Request(String);

#[derive(Debug)]
struct ReportId(String);

#[derive(Debug, PartialEq, Eq)]
struct Failure(&'static str);

/// 使用原始请求执行, 报告独立持有本次 Attempt 的关联与结果.
#[test]
fn report_preserves_attempt_and_result() {
    let mut record = OperationRecord::new(Operation::new(
        String::from("operation-1"),
        Request(String::from("request")),
    ));
    let attempt = record.create_attempt(1).expect("初次创建 Attempt 应成功");
    assert_eq!(attempt.operation_id(), "operation-1");
    assert_eq!(attempt.id(), &1);

    let result = record.operation().request().0.to_uppercase();
    let report = Report::new(ReportId(String::from("report-1")), attempt, result);
    assert_eq!(record.state(), &OperationState::AwaitingReport(1));
    record
        .accept_report(&report.association())
        .expect("本次 Attempt 的报告应被接受");
    assert_eq!(record.state(), &OperationState::Ready);
    assert_eq!(record.operation().request().0, "request");
    record.close().expect("接受报告后应能结束");
    drop(record);

    assert_eq!(report.id().0, "report-1");
    assert_eq!(report.operation_id(), "operation-1");
    assert_eq!(report.attempt_id(), &1);
    assert_eq!(report.result(), "REQUEST");
    assert_eq!(report.into_result(), "REQUEST");
}

/// 失败结果仍可被接受, 重试后旧报告不能解除新的等待.
#[test]
fn failed_report_can_be_accepted_and_retried() {
    let mut record = OperationRecord::new(Operation::new("operation-1", Request(String::new())));
    let attempt = record.create_attempt(1).expect("初次创建 Attempt 应成功");
    let failed = Report::new(
        ReportId(String::from("report-1")),
        attempt,
        Err::<u32, _>(Failure("failed")),
    );

    record
        .accept_report(&failed.association())
        .expect("结果失败不影响关联匹配的报告被接受");
    assert_eq!(record.state(), &OperationState::Ready);
    assert_eq!(failed.result(), &Err(Failure("failed")));

    let attempt = record.create_attempt(2).expect("接受失败报告后应能重试");
    let completed = Report::new(
        ReportId(String::from("report-2")),
        attempt,
        Ok::<_, Failure>(42),
    );
    assert_eq!(
        record.accept_report(&failed.association()),
        Err(AcceptReportError::AttemptMismatch),
    );
    assert_eq!(record.state(), &OperationState::AwaitingReport(2));
    record
        .accept_report(&completed.association())
        .expect("重试报告应被接受");
    assert_eq!(record.state(), &OperationState::Ready);
    assert_eq!(completed.into_result(), Ok(42));
}

/// 从已有 Attempt 构造报告不要求 Operation 身份或结果支持克隆.
#[test]
fn report_construction_moves_attempt_without_cloning() {
    #[derive(Debug)]
    struct OperationId(&'static str);

    let (attempt, _) = OperationState::Ready
        .create_attempt(OperationId("operation-1"), 1)
        .expect("内核创建 Attempt 不要求 Operation 身份支持克隆");
    let report = Report::new(
        ReportId(String::from("report-1")),
        attempt,
        Failure("result"),
    );
    assert_eq!(report.operation_id().0, "operation-1");
    assert_eq!(report.attempt_id(), &1);
    assert_eq!(report.id().0, "report-1");
    assert_eq!(report.into_result(), Failure("result"));
}
