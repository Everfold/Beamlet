use beamlet_kernel::{AcceptReportError, CloseOperationError, OperationState, ReportAssociation};
use beamlet_operation::{Operation, OperationRecord};

#[derive(Debug, PartialEq, Eq)]
struct Request(String);

/// 完整生命周期自动保存状态, 并保留原始请求与可独立持有的 Attempt.
#[test]
fn lifecycle_preserves_request_and_saves_state() {
    let mut record = OperationRecord::new(Operation::new(
        String::from("operation-1"),
        Request(String::from("request")),
    ));
    let first = record.create_attempt(1).expect("初次创建 Attempt 应成功");
    assert_eq!(first.operation_id(), record.operation().id());
    assert_eq!(record.state(), &OperationState::AwaitingReport(1));

    record
        .accept_report(&ReportAssociation {
            operation_id: String::from("operation-1"),
            attempt_id: 1,
        })
        .expect("关联匹配的报告应被接受");
    assert_eq!(record.state(), &OperationState::Ready);

    let second = record.create_attempt(2).expect("接受报告后应能再次创建");
    assert_eq!(record.state(), &OperationState::AwaitingReport(2));
    assert_eq!(
        record.accept_report(&ReportAssociation {
            operation_id: String::from("operation-1"),
            attempt_id: 1,
        }),
        Err(AcceptReportError::AttemptMismatch),
    );
    assert_eq!(record.state(), &OperationState::AwaitingReport(2));

    let report = ReportAssociation {
        operation_id: String::from("operation-1"),
        attempt_id: 2,
    };
    record.accept_report(&report).expect("当前报告应被接受");
    assert_eq!(
        record.accept_report(&report),
        Err(AcceptReportError::NoPendingAttempt),
    );
    assert_eq!(record.state(), &OperationState::Ready);

    record.close().expect("没有等待报告时应能关闭");
    record.close().expect("重复关闭应成功");
    assert_eq!(record.state(), &OperationState::Closed);
    assert_eq!(record.operation().id(), "operation-1");
    assert_eq!(record.operation().request().0, "request");

    drop(record);
    assert_eq!(first.operation_id(), "operation-1");
    assert_eq!(second.id(), &2);
}

/// 拒绝转换时保留当前状态, 创建失败时允许取回 Attempt 身份.
#[test]
fn rejected_transitions_preserve_state() {
    let mut record = OperationRecord::new(Operation::new("operation-1", "request"));
    record.create_attempt(1).expect("初次创建 Attempt 应成功");

    let error = record.create_attempt(2).expect_err("已有等待时应拒绝创建");
    assert_eq!(error.into_ids(), ("operation-1", 2));
    assert_eq!(record.state(), &OperationState::AwaitingReport(1));
    assert_eq!(record.close(), Err(CloseOperationError::AttemptPending));
    assert_eq!(record.state(), &OperationState::AwaitingReport(1));

    for (operation_id, attempt_id, expected) in [
        ("operation-2", 1, AcceptReportError::OperationMismatch),
        ("operation-1", 2, AcceptReportError::AttemptMismatch),
    ] {
        assert_eq!(
            record.accept_report(&ReportAssociation {
                operation_id,
                attempt_id,
            }),
            Err(expected),
            "报告关联 ({operation_id}, {attempt_id}) 应被拒绝",
        );
        assert_eq!(record.state(), &OperationState::AwaitingReport(1));
    }

    record
        .accept_report(&ReportAssociation {
            operation_id: "operation-1",
            attempt_id: 1,
        })
        .expect("关联匹配的报告应被接受");
    record.close().expect("接受报告后应能关闭");
    let error = record.create_attempt(3).expect_err("关闭后应拒绝创建");
    assert_eq!(error.into_ids(), ("operation-1", 3));
    assert_eq!(
        record.accept_report(&ReportAssociation {
            operation_id: "operation-1",
            attempt_id: 1,
        }),
        Err(AcceptReportError::OperationClosed),
    );
    assert_eq!(record.state(), &OperationState::Closed);
}

/// 不同 Operation 的转换互相独立, 报告只能更新所属记录.
#[test]
fn records_use_their_bound_operation_identity() {
    let mut first = OperationRecord::new(Operation::new("operation-1", "same request"));
    let mut second = OperationRecord::new(Operation::new("operation-2", "same request"));
    first.create_attempt(1).expect("第一个记录应能创建 Attempt");
    second
        .create_attempt(2)
        .expect("第二个记录应能创建 Attempt");
    let report = ReportAssociation {
        operation_id: "operation-1",
        attempt_id: 1,
    };

    assert_eq!(
        second.accept_report(&report),
        Err(AcceptReportError::OperationMismatch),
    );
    assert_eq!(second.state(), &OperationState::AwaitingReport(2));
    first.accept_report(&report).expect("报告所属记录应能接受");
    assert_eq!(first.state(), &OperationState::Ready);
    assert_eq!(second.state(), &OperationState::AwaitingReport(2));
}

/// 关闭不要求身份与请求支持克隆或比较.
#[test]
fn closing_does_not_require_clone_or_equality() {
    #[derive(Debug)]
    struct Identity;

    let mut record: OperationRecord<Identity, Identity, Request> =
        OperationRecord::new(Operation::new(Identity, Request(String::from("request"))));
    record.close().expect("关闭不应附加身份约束");
    assert!(matches!(record.state(), OperationState::Closed));
}
