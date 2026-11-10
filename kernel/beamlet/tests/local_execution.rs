use beamlet::{
    Authority, LocalExecution, LocalExecutionError, Worker,
    executor::ExecutorError,
    id::{AttemptId, OperationId},
    memory::{Admission, Limits, OperationPhase, StateError, WaitStatus},
    model::{DispatchAttempt, Effect, ExecutionResult, Message, Operation, SubmitReport},
};
use std::time::Duration;

#[derive(Clone, Debug, Eq, PartialEq)]
struct Input(u32);

fn operation(id: u128, value: u32) -> Operation<Input> {
    Operation {
        operation_id: id.into(),
        task: Input(value),
    }
}
fn execution(limits: Limits) -> LocalExecution<Input, u64, &'static str> {
    let authority = Authority::new(2.into(), vec![3.into(), 4.into()], limits).unwrap();
    let worker = Worker::new(2.into(), vec![3.into(), 4.into()], limits).unwrap();
    LocalExecution::from_parts(authority, worker)
}
async fn execute(attempt: DispatchAttempt<Input>) -> SubmitReport<u64, &'static str> {
    SubmitReport {
        report_id: (1000 + attempt.attempt_id.as_u128()).into(),
        operation_id: attempt.operation.operation_id,
        attempt_id: attempt.attempt_id,
        effect: Effect::None,
        result: ExecutionResult::Succeeded {
            output: u64::from(attempt.operation.task.0) * 2,
        },
    }
}

#[tokio::test]
async fn application_intents_use_one_authorization_and_query_their_own_results() {
    let mut execution = execution(Limits::default());
    let first = execution.submit(operation(1, 21), 1.into()).unwrap();
    assert_eq!(first.admission, Admission::New);
    let duplicate = execution.submit(operation(1, 21), 99.into()).unwrap();
    assert_eq!(duplicate.admission, Admission::Duplicate);
    assert_eq!(first.dispatch, duplicate.dispatch);
    assert!(matches!(
        execution.submit(operation(1, 22), 3.into()),
        Err(LocalExecutionError::State(StateError::OperationConflict))
    ));
    execution.submit(operation(2, 7), 2.into()).unwrap();
    let mut received = Vec::new();
    while let Some(delivery) = execution.next_report(execute).await.unwrap() {
        let Message::SubmitReport(report) = delivery.message else {
            panic!("expected a report");
        };
        received.push((report.operation_id.as_u128(), report.result));
    }
    received.sort_by_key(|value| value.0);
    assert_eq!(
        received,
        vec![
            (1, ExecutionResult::Succeeded { output: 42 }),
            (2, ExecutionResult::Succeeded { output: 14 }),
        ]
    );
    for id in [1, 2] {
        let view = execution.operation(id.into()).unwrap();
        assert_eq!(view.phase, OperationPhase::Reported);
        let retained = execution.report(view.adopted_report.unwrap()).unwrap();
        assert!(
            matches!(&retained.message, Message::SubmitReport(report) if report.operation_id == OperationId::from(id))
        );
    }
}

#[tokio::test(start_paused = true)]
async fn cancelled_execution_wait_keeps_the_original_execution_and_adopts_late_evidence() {
    let mut execution = execution(Limits::default());
    execution.submit(operation(1, 21), 1.into()).unwrap();
    async fn delayed(attempt: DispatchAttempt<Input>) -> SubmitReport<u64, &'static str> {
        tokio::time::sleep(Duration::from_millis(100)).await;
        execute(attempt).await
    }
    let started = tokio::time::Instant::now();
    assert!(
        tokio::time::timeout(Duration::from_millis(10), execution.next_report(delayed))
            .await
            .is_err()
    );
    execution.mark_wait_timeout(1.into()).unwrap();
    let (authority, worker) = execution.into_parts();
    assert_eq!(
        worker.pending(),
        1,
        "taking roles must preserve the running task"
    );
    let mut execution = LocalExecution::from_parts(authority, worker);
    let duplicate = execution.submit(operation(1, 21), 99.into()).unwrap();
    assert_eq!(duplicate.admission, Admission::Duplicate);
    let late = execution.next_report(delayed).await.unwrap().unwrap();
    assert_eq!(started.elapsed(), Duration::from_millis(100));
    assert!(
        matches!(late.message, Message::SubmitReport(report) if report.attempt_id == AttemptId::from(1))
    );
    let view = execution.operation(1.into()).unwrap();
    assert_eq!(view.phase, OperationPhase::Reported);
    assert_eq!(view.wait_status, WaitStatus::TimedOut);
    assert!(execution.next_report(delayed).await.unwrap().is_none());
}

#[tokio::test]
async fn worker_refusal_retains_authority_acceptance_without_automatic_redelivery() {
    let limits = Limits {
        queue_capacity: 1,
        ..Limits::default()
    };
    let mut execution =
        LocalExecution::<Input, u64, &'static str>::new(2.into(), vec![3.into()], limits).unwrap();
    execution.submit(operation(1, 21), 1.into()).unwrap();
    assert!(matches!(
        execution.submit(operation(2, 7), 2.into()),
        Err(LocalExecutionError::State(StateError::QueueFull))
    ));
    assert_eq!(
        execution.operation(2.into()).unwrap().phase,
        OperationPhase::Accepted
    );
    assert_eq!(
        execution
            .submit(operation(2, 7), 99.into())
            .unwrap()
            .admission,
        Admission::Duplicate
    );
    execution.next_report(execute).await.unwrap().unwrap();
    assert!(execution.next_report(execute).await.unwrap().is_none());
    let view = execution.operation(2.into()).unwrap();
    assert_eq!(view.phase, OperationPhase::Accepted);
    assert!(view.adopted_report.is_none());
    assert!(
        matches!(&view.dispatch.message, Message::DispatchAttempt(attempt) if attempt.attempt_id == AttemptId::from(2))
    );
}

#[tokio::test]
async fn invalid_executor_evidence_is_not_turned_into_an_ordinary_failure() {
    let mut execution = execution(Limits::default());
    execution.submit(operation(1, 21), 1.into()).unwrap();
    async fn incorrect(attempt: DispatchAttempt<Input>) -> SubmitReport<u64, &'static str> {
        let mut report = execute(attempt).await;
        report.operation_id = 99.into();
        report
    }
    assert!(matches!(
        execution.next_report(incorrect).await,
        Err(LocalExecutionError::Executor(ExecutorError::State(
            StateError::InvalidBinding
        )))
    ));
    let view = execution.operation(1.into()).unwrap();
    assert_eq!(view.phase, OperationPhase::Dispatched);
    assert!(view.adopted_report.is_none());
}

#[test]
fn explicit_assembly_preserves_queued_work_without_starting_a_runtime() {
    let mut authority =
        Authority::<Input, u64, &'static str>::new(2.into(), vec![3.into()], Limits::default())
            .unwrap();
    let worker_limits = Limits {
        queue_capacity: 1,
        ..Limits::default()
    };
    let mut worker = Worker::new(2.into(), vec![3.into()], worker_limits).unwrap();
    let accepted = authority.submit(operation(1, 21), 1.into()).unwrap();
    worker.receive_dispatch(accepted.dispatch).unwrap();
    authority.mark_dispatched(1.into()).unwrap();

    let mut execution = LocalExecution::from_parts(authority, worker);
    assert!(matches!(
        execution.submit(operation(2, 7), 2.into()),
        Err(LocalExecutionError::State(StateError::QueueFull))
    ));
    let (authority, worker) = execution.into_parts();
    assert_eq!(
        worker.pending(),
        1,
        "assembly must retain the original queue and its limit"
    );
    assert_eq!(
        authority.operation(1.into()).unwrap().phase,
        OperationPhase::Dispatched
    );
    assert_eq!(
        authority.operation(2.into()).unwrap().phase,
        OperationPhase::Accepted
    );
}

#[tokio::test]
async fn assembly_does_not_rewrite_role_bindings_or_execute_a_misrouted_attempt() {
    for (worker_authority, worker_instance) in [(9, 3), (2, 4)] {
        let authority = Authority::new(2.into(), vec![3.into()], Limits::default()).unwrap();
        let worker = Worker::new(
            worker_authority.into(),
            vec![worker_instance.into()],
            Limits::default(),
        )
        .unwrap();
        let mut execution = LocalExecution::from_parts(authority, worker);
        let error = execution.submit(operation(1, 21), 1.into()).unwrap_err();
        assert!(matches!(
            (worker_authority, error),
            (
                9,
                LocalExecutionError::State(StateError::UntrustedAuthority)
            ) | (2, LocalExecutionError::State(StateError::WrongTarget))
        ));
        assert_eq!(
            execution.operation(1.into()).unwrap().phase,
            OperationPhase::Accepted
        );
        assert!(execution.next_report(execute).await.unwrap().is_none());
        let (_, worker) = execution.into_parts();
        assert_eq!(
            worker.pending(),
            0,
            "invalid bindings must not enqueue execution"
        );
    }
}
