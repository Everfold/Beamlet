use beamlet_kernel::{AcceptReportError, Operation, Report};

/// 内核或台账测试的原始请求，故意不实现 Clone，以验证所有权保留。
#[derive(Debug)]
struct Request(
    /// 原始请求文本，失败时必须原样归还或继续保留。
    String,
);

/// 报告接受测试中的自定义报告身份，避免把身份固定为字符串。
#[derive(Debug)]
struct ReportId(
    /// 测试报告的静态名称。
    &'static str,
);

/// 作为 Report 业务结果保存的失败值；业务失败不等于报告关联失败。
#[derive(Debug, PartialEq, Eq)]
struct Failure(
    /// 测试业务失败的说明文本。
    &'static str,
);

/// 验证内核借用完整报告接受结果，无需克隆原请求或结果。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
#[test]
fn accepts_complete_report_without_cloning_request_or_result() {
    let operation = Operation::new("operation-1", Request(String::from("request")));
    let (operation, attempt) = operation.create_attempt("attempt-1");
    let report = Report::new(ReportId("report-1"), attempt, Failure("result"));

    let operation = operation.accept_report(&report).expect("匹配报告应被接受");

    assert_eq!(operation.request().0, "request");

    // 接受报告后可以改用另一种 Attempt 身份类型.
    let (operation, _) = operation.create_attempt(2_u64);
    assert_eq!(operation.pending_attempt_id(), &2);
    drop(operation);

    // Report 独立持有身份和结果, 不借用 Authority 的 Operation.
    assert_eq!(report.id().0, "report-1");
    assert_eq!(report.operation_id(), &"operation-1");
    assert_eq!(report.attempt_id(), &"attempt-1");
    assert_eq!(report.result(), &Failure("result"));
    assert_eq!(report.into_result(), Failure("result"));
}

/// 验证 Operation 不匹配时错误归还原始等待对象。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
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

/// 验证一次报告接受后可再签发授权，同时拒绝旧授权报告。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
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
    let closed = operation.close().close();
    assert_eq!(closed.id(), &"operation-1");
    assert_eq!(closed.request(), &"request");
    drop(closed);

    assert_eq!(completed.into_result(), Ok(42));
}
