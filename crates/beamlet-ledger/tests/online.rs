use beamlet_kernel::OperationStatus;
use beamlet_ledger::{Ledger, LedgerError};
use beamlet_primitives::{Attempt, Report};

#[derive(Debug)]
struct Request(String);

/// 多个 Operation 的报告交错到达时, 路由只推进对应对象, 请求无需 Clone.
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

/// 所有状态拒绝都不消耗授权身份, 已签发身份在其他 Operation 中也不能重用.
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
