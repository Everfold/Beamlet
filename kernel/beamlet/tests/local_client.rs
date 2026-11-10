use beamlet::{
    Authority, LocalClient, LocalClientError, LocalExecution, Worker,
    id::{AttemptId, OperationId},
    memory::{Admission, Limits, StateError},
    model::{Delivery, DispatchAttempt, ExecutionResult, ImpactStatus, Message, SubmitReport},
    state::{OperationPhase, WaitStatus, WorkerPhase},
};
use std::{cell::Cell, time::Duration};

fn execution(limits: Limits) -> LocalExecution<String, usize, String> {
    LocalExecution::new(2.into(), vec![3.into()], limits).unwrap()
}

fn no_allocation() -> (OperationId, AttemptId) {
    panic!("reading or repeating a submission must not allocate execution identities")
}

async fn execute(attempt: DispatchAttempt<String>) -> SubmitReport<usize, String> {
    SubmitReport {
        report_id: (1000 + attempt.attempt_id.as_u128()).into(),
        operation_id: attempt.operation.operation_id,
        attempt_id: attempt.attempt_id,
        impact: ImpactStatus::None,
        result: ExecutionResult::Succeeded {
            output: attempt.operation.task.len(),
        },
    }
}

#[tokio::test]
async fn lost_receipt_is_recovered_after_rebinding_without_a_second_execution() {
    let mut execution = execution(Limits::default());
    let allocations = Cell::new(0_u128);
    {
        let mut client = execution.client(|| {
            let index = allocations.get();
            allocations.set(index + 1);
            ((10 + index).into(), (20 + index).into())
        });
        // The application retains its own key, but loses the receipt and execution IDs.
        client.submit(7.into(), "hello".to_owned()).unwrap();
        let independent = client.submit(8.into(), "hello".to_owned()).unwrap();
        assert_eq!(independent.admission, Admission::New);
        assert_eq!(independent.operation.binding.operation_id, 11.into());
    }
    let original = {
        let mut client = execution.client(no_allocation);
        let recovered = client.query_submission(7.into()).unwrap();
        assert_eq!(recovered.binding.operation_id, 10.into());
        assert_eq!(recovered.binding.attempt_id, 20.into());
        assert_eq!(recovered.phase, OperationPhase::Dispatched);
        assert_eq!(client.query_operation(10.into()).unwrap(), recovered);
        let duplicate = client.submit(7.into(), "hello".to_owned()).unwrap();
        assert_eq!(duplicate.admission, Admission::Duplicate);
        assert_eq!(duplicate.operation, recovered);
        assert!(matches!(
            client.submit(7.into(), "changed".to_owned()),
            Err(LocalClientError::Submission(StateError::SubmissionConflict))
        ));
        recovered
    };
    assert_eq!(allocations.get(), 2);
    let mut reports = 0;
    while execution.next_report(execute).await.unwrap().is_some() {
        reports += 1;
    }
    assert_eq!(reports, 2, "only distinct keys may cause executions");
    let mut client = execution.client(no_allocation);
    let completed = client.query_submission(7.into()).unwrap();
    assert_eq!(completed.binding, original.binding);
    assert_eq!(completed.phase, OperationPhase::Reported);
    assert_eq!(
        original.phase,
        OperationPhase::Dispatched,
        "snapshots stay detached"
    );
    let mut modified = completed.clone();
    modified.task.clear();
    assert_eq!(client.query_operation(10.into()).unwrap(), completed);
    let report = client
        .query_report(completed.adopted_report.unwrap())
        .unwrap();
    assert_eq!(
        report.authority_instance_id,
        original.binding.authority_instance_id
    );
    assert_eq!(
        report.worker_instance_id,
        original.binding.worker_instance_id
    );
    assert_eq!(report.report.operation_id, original.binding.operation_id);
    assert_eq!(report.report.attempt_id, original.binding.attempt_id);
    assert_eq!(
        report.report.result,
        ExecutionResult::Succeeded { output: 5 }
    );
    assert_eq!(
        client
            .submit(7.into(), "hello".to_owned())
            .unwrap()
            .operation,
        completed
    );
}

#[test]
fn unknown_queries_do_not_reserve_keys_or_consume_capacity_and_are_authority_scoped() {
    let limits = Limits {
        max_records: 1,
        ..Limits::default()
    };
    let mut execution = execution(limits);
    {
        let client = execution.client(no_allocation);
        assert!(
            matches!(client.query_submission(7.into()), Err(LocalClientError::SubmissionNotFound(key)) if key == 7.into())
        );
        assert!(
            matches!(client.query_operation(10.into()), Err(LocalClientError::OperationNotFound(id)) if id == 10.into())
        );
        assert!(
            matches!(client.query_report(1020.into()), Err(LocalClientError::ReportNotFound(id)) if id == 1020.into())
        );
    }
    execution
        .client(|| (10.into(), 20.into()))
        .submit(7.into(), "hello".to_owned())
        .unwrap();
    {
        let mut client = execution.client(no_allocation);
        assert!(matches!(
            client.submit(8.into(), "hello".to_owned()),
            Err(LocalClientError::Submission(StateError::Capacity))
        ));
        assert!(matches!(
            client.query_submission(8.into()),
            Err(LocalClientError::SubmissionNotFound(_))
        ));
        assert_eq!(
            client
                .query_submission(7.into())
                .unwrap()
                .binding
                .operation_id,
            10.into()
        );
    }
    let mut other_authority = Authority::new(4.into(), vec![5.into()], limits).unwrap();
    let mut other_worker =
        Worker::<String, usize, String>::new(4.into(), vec![5.into()], limits).unwrap();
    let client = LocalClient::new(&mut other_authority, &mut other_worker, no_allocation);
    assert!(matches!(
        client.query_submission(7.into()),
        Err(LocalClientError::SubmissionNotFound(_))
    ));
    assert!(matches!(
        client.query_operation(10.into()),
        Err(LocalClientError::OperationNotFound(_))
    ));
    let (authority, worker) = execution.into_parts();
    assert_eq!(
        worker
            .retained_dispatch(&authority.operation(10.into()).unwrap().dispatch)
            .unwrap(),
        Some(WorkerPhase::Queued),
        "submission and queries do not start the queued task"
    );
}

#[tokio::test]
async fn delivery_refusal_exposes_accepted_binding_and_repeating_does_not_redeliver() {
    let limits = Limits {
        queue_capacity: 1,
        ..Limits::default()
    };
    let mut execution = execution(limits);
    execution
        .client(|| (10.into(), 20.into()))
        .submit(7.into(), "first".to_owned())
        .unwrap();
    let error = execution
        .client(|| (11.into(), 21.into()))
        .submit(8.into(), "second".to_owned())
        .unwrap_err();
    let LocalClientError::Dispatch {
        binding,
        source: StateError::QueueFull,
    } = error
    else {
        panic!("expected a delivery failure with the accepted execution binding");
    };
    assert_eq!(binding.operation_id, 11.into());
    assert_eq!(binding.attempt_id, 21.into());
    execution.next_report(execute).await.unwrap().unwrap();
    {
        let mut client = execution.client(no_allocation);
        let recovered = client.query_submission(8.into()).unwrap();
        assert_eq!(recovered.binding, binding);
        assert_eq!(recovered.phase, OperationPhase::Accepted);
        assert_eq!(recovered.adopted_report, None);
        let duplicate = client.submit(8.into(), "second".to_owned()).unwrap();
        assert_eq!(duplicate.admission, Admission::Duplicate);
        assert_eq!(duplicate.operation, recovered);
    }
    assert!(
        execution.next_report(execute).await.unwrap().is_none(),
        "freed queue space does not authorize automatic redelivery"
    );
}

#[tokio::test]
async fn independent_role_binding_preserves_authority_identity_and_rejects_wrong_worker() {
    let mut authority = Authority::new(2.into(), vec![3.into()], Limits::default()).unwrap();
    let mut worker =
        Worker::<String, usize, String>::new(2.into(), vec![4.into()], Limits::default()).unwrap();
    {
        let mut client = LocalClient::new(&mut authority, &mut worker, || (10.into(), 20.into()));
        let error = client.submit(7.into(), "hello".to_owned()).unwrap_err();
        let LocalClientError::Dispatch {
            binding,
            source: StateError::WrongTarget,
        } = error
        else {
            panic!("expected rejection by the wrong Worker");
        };
        assert_eq!(binding.authority_instance_id, 2.into());
        assert_eq!(binding.worker_instance_id, 3.into());
        assert_eq!(client.query_submission(7.into()).unwrap().binding, binding);
    }
    assert!(worker.next_report(execute).await.unwrap().is_none());
    assert_eq!(
        authority.operation(10.into()).unwrap().phase,
        OperationPhase::Accepted
    );
}

#[tokio::test(start_paused = true)]
async fn rebinding_during_execution_preserves_late_and_additional_report_evidence() {
    async fn delayed(attempt: DispatchAttempt<String>) -> SubmitReport<usize, String> {
        tokio::time::sleep(Duration::from_millis(100)).await;
        execute(attempt).await
    }
    let mut execution = execution(Limits::default());
    execution
        .client(|| (10.into(), 20.into()))
        .submit(7.into(), "hello".to_owned())
        .unwrap();
    let started = tokio::time::Instant::now();
    assert!(
        tokio::time::timeout(Duration::from_millis(10), execution.next_report(delayed))
            .await
            .is_err()
    );
    execution.mark_wait_timeout(10.into()).unwrap();
    {
        let client = execution.client(no_allocation);
        let waiting = client.query_submission(7.into()).unwrap();
        assert_eq!(waiting.phase, OperationPhase::Dispatched);
        assert_eq!(waiting.wait_status, WaitStatus::TimedOut);
        assert_eq!(waiting.adopted_report, None);
    }
    execution.next_report(delayed).await.unwrap().unwrap();
    assert_eq!(started.elapsed(), Duration::from_millis(100));
    let (mut authority, mut worker) = execution.into_parts();
    let additional = SubmitReport {
        report_id: 1021.into(),
        operation_id: 10.into(),
        attempt_id: 20.into(),
        impact: ImpactStatus::Unknown,
        result: ExecutionResult::Failed {
            error: "additional evidence".to_owned(),
        },
    };
    authority
        .receive_report(Delivery::new(
            3.into(),
            2.into(),
            Message::SubmitReport(additional.clone()),
        ))
        .unwrap();
    let client = LocalClient::new(&mut authority, &mut worker, no_allocation);
    let completed = client.query_submission(7.into()).unwrap();
    assert_eq!(completed.phase, OperationPhase::Reported);
    assert_eq!(completed.wait_status, WaitStatus::TimedOut);
    assert_eq!(completed.adopted_report, Some(1020.into()));
    assert_eq!(
        client.query_report(1020.into()).unwrap().report.result,
        ExecutionResult::Succeeded { output: 5 }
    );
    let observed = client.query_report(1021.into()).unwrap();
    assert_eq!(observed.report, additional);
    assert_eq!(observed.authority_instance_id, 2.into());
    assert_eq!(observed.worker_instance_id, 3.into());
}
