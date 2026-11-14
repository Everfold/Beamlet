use beamlet_kernel::{AcceptReportError, Operation, Report};

#[derive(Debug)]
struct Request(String);

#[derive(Debug)]
struct ReportId(&'static str);

#[derive(Debug, PartialEq, Eq)]
struct Failure(&'static str);

/// 接受完整报告后返回 Ready, 原始请求与报告内容保持不变.
#[test]
fn accepts_complete_report_without_cloning_request_or_result() {
    let operation = Operation::new("operation-1", Request(String::from("request")));
    let (operation, attempt) = operation.create_attempt("attempt-1");
    let report = Report::new(ReportId("report-1"), attempt, Failure("result"));

    let operation = operation.accept_report(&report).expect("匹配报告应被接受");
    assert_eq!(operation.request().0, "request");
    assert_eq!(report.id().0, "report-1");
    assert_eq!(report.result(), &Failure("result"));
    let (operation, _) = operation.create_attempt(2);
    assert_eq!(operation.pending_attempt_id(), &2);
}

/// 两个关联都错误时先拒绝 Operation, 并允许取回原对象继续接受正确报告.
#[test]
fn operation_mismatch_returns_original_operation() {
    let (operation, attempt) = Operation::new("operation-1", "request").create_attempt(1);
    let (_, other_attempt) = Operation::new("operation-2", "other").create_attempt(2);
    let wrong_report = Report::new(2, other_attempt, "wrong");
    let error = operation
        .accept_report(&wrong_report)
        .expect_err("其他 Operation 的报告应被拒绝");
    assert!(matches!(
        &error,
        AcceptReportError::OperationMismatch { .. }
    ));
    assert_eq!(error.to_string(), "report belongs to a different operation");

    let operation = error.into_operation();
    assert_eq!(operation.id(), &"operation-1");
    assert_eq!(operation.request(), &"request");
    assert_eq!(operation.pending_attempt_id(), &1);
    let report = Report::new(1, attempt, "correct");
    let operation = operation
        .accept_report(&report)
        .expect("原 Operation 应仍能接受正确报告");
    let _closed = operation.close();
}

/// 失败结果仍可接受并重试, 旧报告不能解除新的等待.
#[test]
fn retries_after_failure_and_rejects_old_report() {
    let (operation, first) = Operation::new("operation-1", "request").create_attempt(1);
    let failed = Report::new(1, first, Err::<u32, _>(Failure("failed")));
    let operation = operation
        .accept_report(&failed)
        .expect("业务失败不影响报告被接受");
    assert_eq!(failed.result(), &Err(Failure("failed")));

    let (operation, second) = operation.create_attempt(2);
    let error = operation
        .accept_report(&failed)
        .expect_err("旧报告应被拒绝");
    assert!(matches!(&error, AcceptReportError::AttemptMismatch { .. }));
    assert_eq!(error.to_string(), "report belongs to a different attempt");
    let operation = error.into_operation();
    assert_eq!(operation.pending_attempt_id(), &2);
    assert_eq!(operation.request(), &"request");

    let completed = Report::new(2, second, Ok::<_, Failure>(42));
    let operation = operation
        .accept_report(&completed)
        .expect("本次报告应被接受");
    let closed = operation.close();
    assert_eq!(closed.id(), &"operation-1");
    assert_eq!(completed.into_result(), Ok(42));
}
