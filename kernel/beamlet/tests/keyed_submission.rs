use beamlet::{
    Authority,
    id::{AttemptId, OperationId},
    memory::{Admission, Limits, OperationPhase, StateError, Submission, WaitStatus},
    model::{
        Delivery, DispatchAttempt, ExecutionResult, ImpactStatus, Message, Operation, SubmitReport,
    },
};

#[derive(Clone, Debug, Eq, PartialEq)]
struct Input {
    command: &'static str,
    arguments: Vec<u32>,
}

fn input(value: u32) -> Input {
    Input {
        command: "compute",
        arguments: vec![value],
    }
}

fn authority(instance: u128, limits: Limits) -> Authority<Input, u32, &'static str> {
    Authority::new(instance.into(), vec![3.into(), 4.into()], limits).unwrap()
}

fn dispatch<'a>(
    submission: &'a Submission<Input, u32, &'static str>,
) -> &'a DispatchAttempt<Input> {
    let Message::DispatchAttempt(dispatch) = &submission.dispatch.message else {
        panic!("expected an execution authorization");
    };
    dispatch
}

fn no_allocation() -> (OperationId, AttemptId) {
    panic!("this request must not allocate execution identities")
}

#[test]
fn same_key_reuses_execution_and_conflicts_do_not_advance_routing() {
    let mut authority = authority(2, Limits::default());
    assert!(
        authority
            .check_submission(7.into(), &input(21))
            .unwrap()
            .is_none()
    );
    let first = authority
        .submit_keyed(7.into(), input(21), || (10.into(), 20.into()))
        .unwrap();
    assert_eq!(first.admission, Admission::New);
    assert_eq!(first.dispatch.target_instance_id, 3.into());

    let duplicate = authority
        .submit_keyed(7.into(), input(21), no_allocation)
        .unwrap();
    assert_eq!(duplicate.admission, Admission::Duplicate);
    assert_eq!(duplicate.dispatch, first.dispatch);
    assert_eq!(
        authority
            .check_submission(7.into(), &input(21))
            .unwrap()
            .unwrap()
            .dispatch,
        first.dispatch
    );

    assert!(matches!(
        authority.check_submission(7.into(), &input(22)),
        Err(StateError::SubmissionConflict)
    ));
    assert!(matches!(
        authority.submit_keyed(7.into(), input(22), no_allocation),
        Err(StateError::SubmissionConflict)
    ));
    let mut other_command = input(21);
    other_command.command = "different";
    assert!(matches!(
        authority.submit_keyed(7.into(), other_command, no_allocation),
        Err(StateError::SubmissionConflict)
    ));

    let next = authority
        .submit_keyed(8.into(), input(21), || (11.into(), 21.into()))
        .unwrap();
    assert_eq!(next.admission, Admission::New);
    assert_eq!(next.dispatch.target_instance_id, 4.into());
    assert_ne!(
        dispatch(&next).operation.operation_id,
        dispatch(&first).operation.operation_id
    );
    assert_ne!(dispatch(&next).attempt_id, dispatch(&first).attempt_id);
}

#[test]
fn each_authority_owns_its_submission_key_namespace() {
    let mut first = authority(1, Limits::default());
    let mut second = authority(2, Limits::default());
    let accepted = first
        .submit_keyed(7.into(), input(21), || (10.into(), 20.into()))
        .unwrap();
    assert!(
        second
            .check_submission(7.into(), &input(22))
            .unwrap()
            .is_none()
    );
    let independent = second
        .submit_keyed(7.into(), input(22), || (11.into(), 21.into()))
        .unwrap();
    assert_eq!(independent.admission, Admission::New);
    assert_ne!(
        accepted.dispatch.source_instance_id,
        independent.dispatch.source_instance_id
    );
    assert_eq!(dispatch(&accepted).operation.task, input(21));
    assert_eq!(dispatch(&independent).operation.task, input(22));
    assert_eq!(
        first
            .submit_keyed(7.into(), input(21), no_allocation)
            .unwrap()
            .dispatch,
        accepted.dispatch
    );
}

#[test]
fn completed_and_timed_out_submissions_remain_queryable_at_capacity() {
    let limits = Limits {
        max_records: 1,
        ..Limits::default()
    };
    let mut authority = authority(2, limits);
    let accepted = authority
        .submit_keyed(7.into(), input(21), || (10.into(), 20.into()))
        .unwrap();
    authority.mark_wait_timeout(10.into()).unwrap();
    let report = Delivery::new(
        3.into(),
        2.into(),
        Message::SubmitReport(SubmitReport {
            report_id: 30.into(),
            operation_id: 10.into(),
            attempt_id: 20.into(),
            impact: ImpactStatus::None,
            result: ExecutionResult::Succeeded { output: 42 },
        }),
    );
    authority.receive_report(report.clone()).unwrap();

    let repeated = authority
        .submit_keyed(7.into(), input(21), no_allocation)
        .unwrap();
    assert_eq!(repeated.admission, Admission::Duplicate);
    assert_eq!(repeated.dispatch, accepted.dispatch);
    let view = authority
        .check_submission(7.into(), &input(21))
        .unwrap()
        .unwrap();
    assert_eq!(view.phase, OperationPhase::Reported);
    assert_eq!(view.wait_status, WaitStatus::TimedOut);
    assert_eq!(
        authority.report(view.adopted_report.unwrap()),
        Some(&report)
    );
    assert!(matches!(
        authority.submit_keyed(7.into(), input(22), no_allocation),
        Err(StateError::SubmissionConflict)
    ));
    assert!(matches!(
        authority.submit_keyed(8.into(), input(21), no_allocation),
        Err(StateError::Capacity)
    ));
    assert!(
        authority
            .check_submission(8.into(), &input(21))
            .unwrap()
            .is_none()
    );
}

#[test]
fn id_collisions_leave_no_key_or_operation_and_do_not_consume_routing() {
    let mut authority = authority(2, Limits::default());
    let first = authority
        .submit(
            Operation {
                operation_id: 10.into(),
                task: input(21),
            },
            20.into(),
        )
        .unwrap();

    // A new key must not alias an unkeyed Operation, even with equal input.
    assert!(matches!(
        authority.submit_keyed(7.into(), input(21), || (10.into(), 21.into())),
        Err(StateError::OperationConflict)
    ));
    assert!(
        authority
            .check_submission(7.into(), &input(21))
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        authority.submit_keyed(7.into(), input(22), || (11.into(), 20.into())),
        Err(StateError::AttemptConflict)
    ));
    assert!(
        authority
            .check_submission(7.into(), &input(22))
            .unwrap()
            .is_none()
    );
    assert!(authority.operation(11.into()).is_none());
    assert_eq!(
        authority.operation(10.into()).unwrap().dispatch,
        first.dispatch
    );

    // A rejected key can subsequently carry another definition and fresh identities.
    let accepted = authority
        .submit_keyed(7.into(), input(23), || (11.into(), 21.into()))
        .unwrap();
    assert_eq!(accepted.admission, Admission::New);
    assert_eq!(accepted.dispatch.target_instance_id, 4.into());
    assert_eq!(dispatch(&accepted).operation.task, input(23));
}

#[test]
fn a_new_key_cannot_alias_an_existing_keyed_execution() {
    let mut authority = authority(2, Limits::default());
    let first = authority
        .submit_keyed(7.into(), input(21), || (10.into(), 20.into()))
        .unwrap();
    assert!(matches!(
        authority.submit_keyed(8.into(), input(21), || (10.into(), 21.into())),
        Err(StateError::OperationConflict)
    ));
    assert!(
        authority
            .check_submission(8.into(), &input(21))
            .unwrap()
            .is_none()
    );
    assert_eq!(
        authority
            .submit_keyed(7.into(), input(21), no_allocation)
            .unwrap()
            .dispatch,
        first.dispatch
    );
}

#[test]
fn preflight_does_not_reserve_space_and_unkeyed_records_share_the_limit() {
    let limits = Limits {
        max_records: 1,
        ..Limits::default()
    };
    let mut authority = authority(2, limits);
    assert!(
        authority
            .check_submission(7.into(), &input(21))
            .unwrap()
            .is_none()
    );
    authority
        .submit(
            Operation {
                operation_id: 10.into(),
                task: input(22),
            },
            20.into(),
        )
        .unwrap();
    assert!(matches!(
        authority.submit_keyed(7.into(), input(21), no_allocation),
        Err(StateError::Capacity)
    ));
    assert!(
        authority
            .check_submission(7.into(), &input(21))
            .unwrap()
            .is_none()
    );
    assert_eq!(
        authority.operation(10.into()).unwrap().phase,
        OperationPhase::Accepted
    );
}
