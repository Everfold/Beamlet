use beamlet_kernel::OperationStatus;
use beamlet_ledger::{Ledger, LedgerError};
use beamlet_primitives::{Attempt, Report};

/// 内核或台账测试的原始请求, 故意不实现 Clone, 以验证所有权保留.
#[derive(Debug)]
struct Request(
    /// 原始请求文本, 失败时必须原样归还或继续保留.
    String,
);

/// 验证台账路由交错报告并保留各 Operation 的原始请求.
///
/// # 返回值
///
/// `()`; 全部断言通过时正常结束, 断言失败或未预期错误会使测试失败.
#[test]
fn routes_interleaved_reports_and_preserves_requests() {
    let mut ledger = Ledger::new();
    ledger
        .create_operation("one", Request("first".into()))
        .unwrap();
    ledger
        .create_operation("two", Request("second".into()))
        .unwrap();
    let first = ledger.create_attempt(&"one", 1).unwrap();
    let second = ledger.create_attempt(&"two", 2).unwrap();

    let report = Report::new(10, second, Err::<(), _>("business failure"));
    ledger.accept_report(&report).unwrap();
    assert_eq!(
        ledger.operation(&"two").unwrap().status(),
        OperationStatus::Ready
    );
    assert_eq!(
        ledger.operation(&"one").unwrap().pending_attempt_id(),
        Some(&1)
    );

    ledger.accept_report(&Report::new(11, first, "ok")).unwrap();
    ledger.close(&"one").unwrap();
    ledger.close(&"one").unwrap();

    let view = ledger.operation(&"one").unwrap();
    assert_eq!(view.id(), &"one");
    assert_eq!(view.status(), OperationStatus::Closed);
    assert_eq!(view.pending_attempt_id(), None);
    assert_eq!(view.request().0, "first");
    assert_eq!(ledger.operation(&"two").unwrap().request().0, "second");
    assert_eq!(report.result(), &Err("business failure"));
}

/// 验证报告被拒绝后台账仍保留原等待 Operation.
///
/// # 返回值
///
/// `()`; 全部断言通过时正常结束, 断言失败或未预期错误会使测试失败.
#[test]
fn rejected_reports_preserve_waiting_operation() {
    let mut ledger = Ledger::new();
    ledger.create_operation(1, "request").unwrap();
    let first = ledger.create_attempt(&1, 7).unwrap();

    let old = Report::new(1, first, "old");
    ledger.accept_report(&old).unwrap();
    assert_eq!(
        ledger.accept_report(&old),
        Err(LedgerError::NoPendingAttempt)
    );
    assert_eq!(
        ledger.create_attempt(&1, 7),
        Err(LedgerError::AttemptIdReused)
    );

    let second = ledger.create_attempt(&1, 8).unwrap();
    assert_eq!(ledger.accept_report(&old), Err(LedgerError::ReportMismatch));

    let foreign = Report::new(2, Attempt::from_parts(99, 8), "unknown");
    assert_eq!(
        ledger.accept_report(&foreign),
        Err(LedgerError::OperationNotFound)
    );
    assert_eq!(ledger.operation(&1).unwrap().pending_attempt_id(), Some(&8));

    ledger
        .accept_report(&Report::new(3, second, "current"))
        .unwrap();
    ledger.close(&1).unwrap();
    assert_eq!(
        ledger.accept_report(&old),
        Err(LedgerError::OperationClosed)
    );
}

/// 验证重复创建错误归还被拒绝请求的所有权.
///
/// # 返回值
///
/// `()`; 全部断言通过时正常结束, 断言失败或未预期错误会使测试失败.
#[test]
fn duplicate_operation_returns_owned_request() {
    let mut ledger: Ledger<String, u64, Request> = Ledger::new();
    ledger
        .create_operation("one".into(), Request("original".into()))
        .unwrap();

    let error = ledger
        .create_operation("one".into(), Request("rejected".into()))
        .unwrap_err();
    let (id, request) = error.into_parts();

    assert_eq!(id, "one");
    assert_eq!(request.0, "rejected");
    assert_eq!(ledger.operation(&id).unwrap().request().0, "original");
}

/// 验证台账状态约束和全台账 Attempt 身份唯一性.
///
/// # 返回值
///
/// `()`; 全部断言通过时正常结束, 断言失败或未预期错误会使测试失败.
#[test]
fn enforces_states_and_attempt_identity_uniqueness() {
    let mut ledger = Ledger::new();
    assert!(ledger.operation(&1).is_none());
    assert_eq!(ledger.close(&1), Err(LedgerError::OperationNotFound));
    assert_eq!(
        ledger.create_attempt(&1, 7),
        Err(LedgerError::OperationNotFound)
    );

    ledger.create_operation(1, ()).unwrap();
    ledger.create_operation(2, ()).unwrap();
    let first = ledger.create_attempt(&1, 7).unwrap();
    assert_eq!(ledger.close(&1), Err(LedgerError::AttemptPending));
    assert_eq!(
        ledger.create_attempt(&1, 8),
        Err(LedgerError::AttemptPending)
    );
    assert_eq!(
        ledger.create_attempt(&2, 7),
        Err(LedgerError::AttemptIdReused)
    );

    let second = ledger.create_attempt(&2, 8).unwrap();
    ledger.accept_report(&Report::new(1, first, ())).unwrap();
    ledger.accept_report(&Report::new(2, second, ())).unwrap();

    ledger.close(&1).unwrap();
    assert_eq!(
        ledger.create_attempt(&1, 9),
        Err(LedgerError::OperationClosed)
    );
    assert!(ledger.create_attempt(&2, 9).is_ok());
}
