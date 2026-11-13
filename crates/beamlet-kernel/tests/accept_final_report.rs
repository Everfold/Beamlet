use beamlet_kernel::{AcceptReportError, OperationState, ReportAssociation};

#[derive(Debug, PartialEq, Eq)]
struct OperationId(&'static str);

#[derive(Debug, PartialEq, Eq)]
struct AttemptId(&'static str);

/// 匹配的报告返回 Ready, 原输入不变.
#[test]
fn accepts_matching_report_without_uuid_or_cloning() {
    let operation_id = OperationId("operation-1");
    let state = OperationState::AwaitingReport(AttemptId("attempt-1"));
    let report = ReportAssociation {
        operation_id: OperationId("operation-1"),
        attempt_id: AttemptId("attempt-1"),
    };

    let next = state.accept_final_report(&operation_id, &report);

    assert_eq!(next, Ok(OperationState::Ready));
    assert_eq!(
        state,
        OperationState::AwaitingReport(AttemptId("attempt-1"))
    );
    assert_eq!(report.attempt_id, AttemptId("attempt-1"));
}

/// 状态或关联不合法时拒绝报告, 原状态不变.
#[test]
fn rejects_invalid_state_or_association() {
    let cases = [
        (
            OperationState::Ready,
            OperationId("operation-1"),
            AttemptId("attempt-1"),
            AcceptReportError::NoPendingAttempt,
        ),
        (
            OperationState::Closed,
            OperationId("operation-1"),
            AttemptId("attempt-1"),
            AcceptReportError::OperationClosed,
        ),
        (
            OperationState::AwaitingReport(AttemptId("attempt-1")),
            OperationId("operation-2"),
            AttemptId("attempt-1"),
            AcceptReportError::OperationMismatch,
        ),
        (
            OperationState::AwaitingReport(AttemptId("attempt-1")),
            OperationId("operation-1"),
            AttemptId("attempt-2"),
            AcceptReportError::AttemptMismatch,
        ),
    ];

    for (state, report_operation, report_attempt, expected) in cases {
        let operation_id = OperationId("operation-1");
        let report = ReportAssociation {
            operation_id: report_operation,
            attempt_id: report_attempt,
        };
        let before = format!("{state:?}");

        assert_eq!(
            state.accept_final_report(&operation_id, &report),
            Err(expected),
            "状态或关联错误应被拒绝: {state:?}, {report:?}",
        );
        assert_eq!(format!("{state:?}"), before, "拒绝报告不应改变原状态");
    }
}

/// 更新状态后, 再次提交同一报告会被拒绝.
#[test]
fn rejects_report_after_pending_attempt_is_cleared() {
    let operation_id = OperationId("operation-1");
    let state = OperationState::AwaitingReport(AttemptId("attempt-1"));
    let report = ReportAssociation {
        operation_id: OperationId("operation-1"),
        attempt_id: AttemptId("attempt-1"),
    };
    let state = state
        .accept_final_report(&operation_id, &report)
        .expect("匹配的最终报告应被接受");

    assert_eq!(
        state.accept_final_report(&operation_id, &report),
        Err(AcceptReportError::NoPendingAttempt),
    );
}
