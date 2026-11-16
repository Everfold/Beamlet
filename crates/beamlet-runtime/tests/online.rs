use beamlet_kernel::OperationStatus;
use beamlet_primitives::{Attempt, Report};
use beamlet_runtime::{Runtime, RuntimeError};

#[derive(Debug)]
struct Request(String);

/// 多个 Operation 的报告交错到达时, 路由只推进对应对象, 请求无需 Clone.
#[test]
fn routes_interleaved_reports_and_preserves_requests() {
    let mut runtime = Runtime::new();
    runtime
        .create_operation("one", Request("first".into()))
        .unwrap();
    runtime
        .create_operation("two", Request("second".into()))
        .unwrap();
    let first = runtime.create_attempt(&"one", 1).unwrap();
    let second = runtime.create_attempt(&"two", 2).unwrap();

    let report = Report::new(10, second, Err::<(), _>("business failure"));
    runtime.accept_report(&report).unwrap();
    assert_eq!(
        runtime.operation(&"two").unwrap().status(),
        OperationStatus::Ready
    );
    assert_eq!(
        runtime.operation(&"one").unwrap().pending_attempt_id(),
        Some(&1)
    );

    runtime
        .accept_report(&Report::new(11, first, "ok"))
        .unwrap();
    runtime.close(&"one").unwrap();
    runtime.close(&"one").unwrap();

    let view = runtime.operation(&"one").unwrap();
    assert_eq!(view.id(), &"one");
    assert_eq!(view.status(), OperationStatus::Closed);
    assert_eq!(view.pending_attempt_id(), None);
    assert_eq!(view.request().0, "first");
    assert_eq!(runtime.operation(&"two").unwrap().request().0, "second");
    assert_eq!(report.result(), &Err("business failure"));
}

#[test]
fn rejected_reports_preserve_waiting_operation() {
    let mut runtime = Runtime::new();
    runtime.create_operation(1, "request").unwrap();
    let first = runtime.create_attempt(&1, 7).unwrap();

    let old = Report::new(1, first, "old");
    runtime.accept_report(&old).unwrap();
    assert_eq!(
        runtime.accept_report(&old),
        Err(RuntimeError::NoPendingAttempt)
    );
    assert_eq!(
        runtime.create_attempt(&1, 7),
        Err(RuntimeError::AttemptIdReused)
    );

    let second = runtime.create_attempt(&1, 8).unwrap();
    assert_eq!(
        runtime.accept_report(&old),
        Err(RuntimeError::ReportMismatch)
    );

    let foreign = Report::new(2, Attempt::from_parts(99, 8), "unknown");
    assert_eq!(
        runtime.accept_report(&foreign),
        Err(RuntimeError::OperationNotFound)
    );
    assert_eq!(
        runtime.operation(&1).unwrap().pending_attempt_id(),
        Some(&8)
    );

    runtime
        .accept_report(&Report::new(3, second, "current"))
        .unwrap();
    runtime.close(&1).unwrap();
    assert_eq!(
        runtime.accept_report(&old),
        Err(RuntimeError::OperationClosed)
    );
}

#[test]
fn duplicate_operation_returns_owned_request() {
    let mut runtime: Runtime<String, u64, Request> = Runtime::new();
    runtime
        .create_operation("one".into(), Request("original".into()))
        .unwrap();

    let error = runtime
        .create_operation("one".into(), Request("rejected".into()))
        .unwrap_err();
    let (id, request) = error.into_parts();

    assert_eq!(id, "one");
    assert_eq!(request.0, "rejected");
    assert_eq!(runtime.operation(&id).unwrap().request().0, "original");
}

/// 所有状态拒绝都不消耗授权身份, 已签发身份在其他 Operation 中也不能重用.
#[test]
fn enforces_states_and_attempt_identity_uniqueness() {
    let mut runtime = Runtime::new();
    assert!(runtime.operation(&1).is_none());
    assert_eq!(runtime.close(&1), Err(RuntimeError::OperationNotFound));
    assert_eq!(
        runtime.create_attempt(&1, 7),
        Err(RuntimeError::OperationNotFound)
    );

    runtime.create_operation(1, ()).unwrap();
    runtime.create_operation(2, ()).unwrap();
    let first = runtime.create_attempt(&1, 7).unwrap();
    assert_eq!(runtime.close(&1), Err(RuntimeError::AttemptPending));
    assert_eq!(
        runtime.create_attempt(&1, 8),
        Err(RuntimeError::AttemptPending)
    );
    assert_eq!(
        runtime.create_attempt(&2, 7),
        Err(RuntimeError::AttemptIdReused)
    );

    let second = runtime.create_attempt(&2, 8).unwrap();
    runtime.accept_report(&Report::new(1, first, ())).unwrap();
    runtime.accept_report(&Report::new(2, second, ())).unwrap();

    runtime.close(&1).unwrap();
    assert_eq!(
        runtime.create_attempt(&1, 9),
        Err(RuntimeError::OperationClosed)
    );
    assert!(runtime.create_attempt(&2, 9).is_ok());
}
