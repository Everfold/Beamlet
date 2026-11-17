use beamlet_kernel::{AcceptReportError, Operation, OperationStatus};
use beamlet_primitives::{Attempt, Report};

#[derive(Debug, PartialEq, Eq)]
struct Identity(u64);

#[derive(Debug)]
struct Request(String);

#[derive(Debug)]
struct OpaqueIdentity(String);

/// 验证从已保存授权恢复等待状态，不重新签发或克隆输入。
///
/// # 参数
///
/// 无。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
#[test]
fn restores_waiting_without_minting_attempt_or_cloning_inputs() {
    let operation = Operation::restore_awaiting_report(
        Identity(1),
        Request("original request".into()),
        Attempt::from_parts(Identity(1), Identity(7)),
    )
    .expect("匹配身份应恢复成功");

    assert_eq!(operation.status(), OperationStatus::AwaitingReport);
    assert_eq!(operation.pending_attempt_id().0, 7);
    assert_eq!(operation.request().0, "original request");

    let report = Report::new(
        Identity(11),
        Attempt::from_parts(Identity(1), Identity(7)),
        Err::<(), _>("business failed"),
    );
    let ready = operation
        .accept_report(&report)
        .expect("失败结果也是已结束报告");

    assert_eq!(ready.status(), OperationStatus::Ready);
    assert_eq!(ready.id().0, 1);
    assert_eq!(ready.request().0, "original request");
    assert_eq!(ready.close().status(), OperationStatus::Closed);
}

/// 验证恢复时身份不匹配会归还全部输入数据。
///
/// # 参数
///
/// 无。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
#[test]
fn restore_mismatch_returns_all_owned_inputs() {
    let error = Operation::restore_awaiting_report(
        Identity(1),
        Request("request".into()),
        Attempt::from_parts(Identity(2), Identity(7)),
    )
    .expect_err("跨 Operation 授权必须拒绝");
    let (id, request, attempt) = error.into_parts();

    assert_eq!(id.0, 1);
    assert_eq!(request.0, "request");
    assert_eq!(attempt.operation_id().0, 2);
    assert_eq!(attempt.id().0, 7);
}

/// 验证可以使用不透明身份恢复 Closed Operation。
///
/// # 参数
///
/// 无。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
#[test]
fn restores_closed_with_opaque_identity() {
    let operation = Operation::restore_closed(
        OpaqueIdentity("opaque id".into()),
        Request("request".into()),
    );
    assert_eq!(operation.status(), OperationStatus::Closed);

    let closed = operation.close();
    assert_eq!(closed.status(), OperationStatus::Closed);
    assert_eq!(closed.id().0, "opaque id");
    assert_eq!(closed.request().0, "request");
}

/// 验证恢复后的等待对象拒绝旧报告后仍可接受正确报告。
///
/// # 参数
///
/// 无。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
#[test]
fn restored_waiting_rejects_stale_report_and_remains_usable() {
    let operation = Operation::restore_awaiting_report(1, "request", Attempt::from_parts(1, 7))
        .expect("恢复等待状态");
    let old_report = Report::new(10, Attempt::from_parts(1, 6), "old result");
    let error = operation
        .accept_report(&old_report)
        .expect_err("旧授权报告必须被拒绝");
    assert!(matches!(&error, AcceptReportError::AttemptMismatch { .. }));
    let operation = error.into_operation();
    assert_eq!(operation.pending_attempt_id(), &7);

    let report = Report::new(11, Attempt::from_parts(1, 7), "current result");
    let ready = operation
        .accept_report(&report)
        .expect("当前授权报告应被接受");

    assert_eq!(ready.status(), OperationStatus::Ready);
    assert_eq!(report.into_result(), "current result");
}
