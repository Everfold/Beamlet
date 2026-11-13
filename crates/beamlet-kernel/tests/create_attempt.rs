use beamlet_kernel::{AcceptReportError, CreateAttemptError, OperationState, ReportAssociation};

#[derive(Debug, PartialEq, Eq)]
struct OperationId(&'static str);

#[derive(Debug, Clone, PartialEq, Eq)]
struct AttemptId(&'static str);

/// 创建的 Attempt 关联正确, 原状态不变, Operation 身份无需克隆.
#[test]
fn creates_attempt_and_next_state() {
    let state = OperationState::Ready;

    let (attempt, next) = state
        .create_attempt(OperationId("operation-1"), AttemptId("attempt-1"))
        .expect("Ready 应允许创建 Attempt");

    assert_eq!(attempt.id(), &AttemptId("attempt-1"));
    assert_eq!(attempt.operation_id(), &OperationId("operation-1"));
    assert_eq!(next, OperationState::AwaitingReport(AttemptId("attempt-1")));
    assert_eq!(state, OperationState::Ready);
}

/// 等待报告时拒绝创建 Attempt, 并返回传入的身份.
#[test]
fn rejects_creation_while_awaiting_report() {
    let state = OperationState::AwaitingReport(AttemptId("attempt-1"));

    for attempt in ["attempt-1", "attempt-2"] {
        let error = state
            .create_attempt(OperationId("operation-1"), AttemptId(attempt))
            .expect_err("等待报告时应拒绝创建 Attempt");

        assert!(matches!(&error, CreateAttemptError::AttemptPending { .. }));
        assert_eq!(
            error.into_ids(),
            (OperationId("operation-1"), AttemptId(attempt)),
        );
        assert_eq!(
            state,
            OperationState::AwaitingReport(AttemptId("attempt-1")),
        );
    }
}

/// 已结束的 Operation 拒绝创建 Attempt, 并返回传入的身份.
#[test]
fn rejects_creation_after_close() {
    let state = OperationState::Closed;
    let error = state
        .create_attempt(OperationId("operation-1"), AttemptId("attempt-1"))
        .expect_err("已结束的 Operation 应拒绝创建 Attempt");

    assert!(matches!(&error, CreateAttemptError::OperationClosed { .. }));
    assert_eq!(
        error.into_ids(),
        (OperationId("operation-1"), AttemptId("attempt-1")),
    );
    assert_eq!(state, OperationState::Closed);
}

/// 接受报告后可以创建新 Attempt, 旧报告不能解除新的等待.
#[test]
fn accepts_report_and_creates_next_attempt() {
    let (first_attempt, state) = OperationState::Ready
        .create_attempt("operation-1", AttemptId("attempt-1"))
        .expect("Ready 应允许创建 Attempt");
    let first_report = ReportAssociation {
        operation_id: *first_attempt.operation_id(),
        attempt_id: first_attempt.id().clone(),
    };
    let state = state
        .accept_report(first_attempt.operation_id(), &first_report)
        .expect("当前 Attempt 的报告应被接受");
    let (second_attempt, state) = state
        .create_attempt("operation-1", AttemptId("attempt-2"))
        .expect("接受报告后应允许创建新 Attempt");

    assert_eq!(
        state.accept_report(second_attempt.operation_id(), &first_report),
        Err(AcceptReportError::AttemptMismatch),
    );

    let second_report = ReportAssociation {
        operation_id: *second_attempt.operation_id(),
        attempt_id: second_attempt.id().clone(),
    };
    assert_eq!(
        state.accept_report(second_attempt.operation_id(), &second_report),
        Ok(OperationState::Ready),
    );
}
