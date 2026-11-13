use beamlet_kernel::{
    AcceptReportError, CloseOperationError, CreateAttemptError, OperationState, ReportAssociation,
};

#[derive(Debug, PartialEq, Eq)]
struct AttemptId(&'static str);

/// Ready 可以关闭, 原状态不变, 身份无需克隆.
#[test]
fn closes_ready() {
    let state = OperationState::<AttemptId>::Ready;

    assert_eq!(state.close(), Ok(OperationState::Closed));
    assert_eq!(state, OperationState::Ready);
}

/// 重复关闭仍返回 Closed.
#[test]
fn closes_already_closed_operation() {
    let state = OperationState::<AttemptId>::Closed;

    let next = state.close().expect("已关闭的 Operation 应允许重复关闭");

    assert_eq!(next, OperationState::Closed);
    assert_eq!(next.close(), Ok(OperationState::Closed));
}

/// 等待报告时拒绝关闭, 保留当前 Attempt.
#[test]
fn rejects_close_while_awaiting_report() {
    let state = OperationState::AwaitingReport(AttemptId("attempt-1"));

    assert_eq!(state.close(), Err(CloseOperationError::AttemptPending));
    assert_eq!(
        state,
        OperationState::AwaitingReport(AttemptId("attempt-1"))
    );
}

/// 接受最终报告后可以关闭, 关闭后拒绝创建 Attempt 和接受报告.
#[test]
fn closes_after_accepting_final_report() {
    let (attempt, state) = OperationState::Ready
        .create_attempt("operation-1", "attempt-1")
        .expect("Ready 应允许创建 Attempt");
    let report = ReportAssociation {
        operation_id: *attempt.operation_id(),
        attempt_id: *attempt.id(),
    };
    let state = state
        .accept_final_report(attempt.operation_id(), &report)
        .expect("当前 Attempt 的报告应被接受");
    let state = state.close().expect("接受最终报告后应允许关闭");

    assert_eq!(state, OperationState::Closed);
    assert!(matches!(
        state.create_attempt("operation-1", "attempt-2"),
        Err(CreateAttemptError::OperationClosed { .. }),
    ));
    assert_eq!(
        state.accept_final_report(attempt.operation_id(), &report),
        Err(AcceptReportError::OperationClosed),
    );
}
