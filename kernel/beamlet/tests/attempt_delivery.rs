use beamlet::{
    Authority, ExecutionBinding, Worker,
    attempt_delivery::{
        Error, Event, Limits, Outbox, RetainError, SendError, Status, Transmission,
    },
    memory::Limits as LedgerLimits,
    model::{
        Delivery, DispatchAttempt, ExecutionResult, ImpactStatus, Message, Operation, SubmitReport,
    },
};
use std::{cell::Cell, convert::Infallible, io, time::Duration};
use tokio::time::{Instant, advance};

type Dispatch = Delivery<String, String, String>;

fn dispatch(operation: u128, attempt: u128) -> Dispatch {
    Delivery::new(
        2.into(),
        3.into(),
        Message::DispatchAttempt(DispatchAttempt {
            attempt_id: attempt.into(),
            operation: Operation {
                operation_id: operation.into(),
                task: "original task".to_owned(),
            },
        }),
    )
}

fn outbox(limits: Limits) -> Outbox<String> {
    Outbox::new(2.into(), limits).unwrap()
}
fn retain(outbox: &mut Outbox<String>, dispatch: &Dispatch) {
    outbox
        .retain(dispatch, |_| {
            Ok::<_, Infallible>(b"original encoding".to_vec())
        })
        .unwrap();
}
fn next(outbox: &mut Outbox<String>) -> Transmission {
    let Some(Event::Send(transmission)) = outbox.next_event() else {
        panic!("expected one original transmission")
    };
    transmission
}
fn binding() -> ExecutionBinding {
    ExecutionBinding {
        authority_instance_id: 2.into(),
        worker_instance_id: 3.into(),
        operation_id: 10.into(),
        attempt_id: 20.into(),
    }
}
fn report(attempt: DispatchAttempt<String>) -> SubmitReport<String, String> {
    SubmitReport {
        report_id: 30.into(),
        operation_id: attempt.operation.operation_id,
        attempt_id: attempt.attempt_id,
        impact: ImpactStatus::Unknown,
        result: ExecutionResult::Failed {
            error: "tool failed".to_owned(),
        },
    }
}

#[tokio::test(start_paused = true)]
async fn identical_bytes_and_binding_use_fixed_send_and_observation_budgets() {
    for mode in 0..3 {
        let mut outbox = outbox(Limits::default());
        let encodings = Cell::new(0);
        outbox
            .retain(&dispatch(10, 20), |_| {
                encodings.set(encodings.get() + 1);
                Ok::<_, Infallible>(b"immutable bytes".to_vec())
            })
            .unwrap();
        let start = Instant::now();
        let mut times = Vec::new();
        for number in 1..=3 {
            let transmission = next(&mut outbox);
            let ticket = transmission.ticket();
            assert_eq!(ticket.binding(), binding());
            assert_eq!(ticket.number(), number);
            assert!(
                outbox.next_event().is_none(),
                "cannot send concurrently for one authorization"
            );
            times.push(start.elapsed());
            let result = transmission
                .send(|worker, bytes| async move {
                    assert_eq!(worker, 3.into());
                    assert_eq!(bytes, b"immutable bytes");
                    match mode {
                        0 => Ok(()),
                        1 => Err(io::Error::other("transport failure")),
                        _ => std::future::pending().await,
                    }
                })
                .await;
            match mode {
                0 => assert!(result.is_ok()),
                1 => assert!(matches!(result, Err(SendError::Transport(_)))),
                _ => assert!(matches!(result, Err(SendError::TimedOut))),
            }
            assert!(outbox.sent(ticket));
            assert!(
                !outbox.sent(ticket),
                "duplicate completion cannot postpone next send"
            );
            outbox
                .retain(&dispatch(10, 20), |_| -> Result<Vec<u8>, Infallible> {
                    panic!("repeated retain cannot encode or renew budget")
                })
                .unwrap();
            assert!(outbox.next_event().is_none());
            advance(Duration::from_secs(1)).await;
        }
        let interval = if mode == 2 { 1500 } else { 1000 };
        assert_eq!(
            times,
            vec![
                Duration::ZERO,
                Duration::from_millis(interval),
                Duration::from_millis(interval * 2)
            ]
        );
        let Some(Event::Exhausted(progress)) = outbox.next_event() else {
            panic!("must report exhausted budget once")
        };
        assert_eq!(progress.sends, 3);
        assert_eq!(progress.status, Status::Unconfirmed);
        assert_eq!(encodings.get(), 1);
        retain(&mut outbox, &dispatch(10, 20));
        assert!(
            outbox.next_event().is_none(),
            "exhaustion cannot be reset by another retain"
        );
        assert!(
            outbox.acknowledge(binding()).unwrap(),
            "late valid receipt remains useful"
        );
        assert_eq!(
            outbox.progress(10.into()).unwrap().status,
            Status::Confirmed
        );
        assert!(!outbox.acknowledge(binding()).unwrap());
    }
}

#[tokio::test(start_paused = true)]
async fn total_deadline_expires_without_send_completion_and_preserves_original_identity() {
    let mut outbox = outbox(Limits {
        max_pending: 1,
        max_records: 2,
    });
    retain(&mut outbox, &dispatch(10, 20));
    let held = next(&mut outbox);
    let ticket = held.ticket();
    assert!(!outbox.has_capacity());
    advance(Duration::from_millis(4999)).await;
    assert!(outbox.next_event().is_none());
    retain(&mut outbox, &dispatch(10, 20));
    advance(Duration::from_millis(1)).await;
    let Some(Event::Exhausted(progress)) = outbox.next_event() else {
        panic!("total deadline must expire even when the application holds a send item")
    };
    assert_eq!(progress.sends, 1);
    assert!(
        outbox.has_capacity(),
        "expired byte storage releases the active slot"
    );
    retain(&mut outbox, &dispatch(11, 21));
    assert!(outbox.sent(ticket));
    assert!(outbox.acknowledge(binding()).unwrap());
    assert_eq!(next(&mut outbox).ticket().binding().operation_id, 11.into());
    assert!(outbox.next_event().is_none());
    assert!(
        !outbox.has_capacity(),
        "completed identity records are not evicted"
    );
}

#[tokio::test(start_paused = true)]
async fn full_receipt_binding_is_required_and_late_send_completion_cannot_reopen_delivery() {
    let mut outbox = outbox(Limits::default());
    retain(&mut outbox, &dispatch(10, 20));
    let first = next(&mut outbox).ticket();
    assert!(outbox.sent(first));
    advance(Duration::from_secs(1)).await;
    let second = next(&mut outbox).ticket();
    assert!(
        !outbox.sent(first),
        "old completion must not release the active second send"
    );
    for bad in [
        ExecutionBinding {
            authority_instance_id: 9.into(),
            ..binding()
        },
        ExecutionBinding {
            worker_instance_id: 9.into(),
            ..binding()
        },
        ExecutionBinding {
            attempt_id: 9.into(),
            ..binding()
        },
    ] {
        assert_eq!(outbox.acknowledge(bad), Err(Error::InvalidBinding));
        assert_eq!(outbox.progress(10.into()).unwrap().status, Status::Active);
    }
    assert_eq!(
        outbox.acknowledge(ExecutionBinding {
            operation_id: 99.into(),
            ..binding()
        }),
        Err(Error::Unknown)
    );
    assert!(outbox.acknowledge(binding()).unwrap());
    assert!(outbox.sent(second));
    advance(Duration::from_secs(10)).await;
    assert!(outbox.next_event().is_none());
    retain(&mut outbox, &dispatch(10, 20));
    assert_eq!(
        outbox.progress(10.into()).unwrap().status,
        Status::Confirmed
    );
    assert!(outbox.next_event().is_none());
}

#[tokio::test(start_paused = true)]
async fn invalid_conflicting_or_unencodable_dispatches_do_not_consume_records() {
    assert!(matches!(
        Outbox::<String>::new(
            2.into(),
            Limits {
                max_records: 0,
                ..Limits::default()
            }
        ),
        Err(Error::InvalidLimits)
    ));
    let mut outbox = outbox(Limits {
        max_records: 2,
        max_pending: 1,
    });
    let original = dispatch(10, 20);
    assert!(matches!(
        outbox.retain(&original, |_| Err::<Vec<u8>, _>(io::Error::other(
            "encode failed"
        ))),
        Err(RetainError::Encoding(_))
    ));
    assert!(outbox.progress(10.into()).is_none());
    assert!(outbox.has_capacity());
    retain(&mut outbox, &original);
    let mut changed = original.clone();
    let Message::DispatchAttempt(attempt) = &mut changed.message else {
        unreachable!()
    };
    attempt.operation.task = "changed".to_owned();
    let mut wrong_worker = original.clone();
    wrong_worker.target_instance_id = 4.into();
    for invalid in [changed, wrong_worker, dispatch(10, 21), dispatch(11, 20)] {
        assert!(matches!(
            outbox.retain(&invalid, |_| -> Result<Vec<u8>, Infallible> {
                panic!("invalid dispatch must fail before encoding")
            }),
            Err(RetainError::State(Error::Conflict))
        ));
    }
    let mut wrong_authority = original.clone();
    wrong_authority.source_instance_id = 9.into();
    assert!(matches!(
        outbox.retain(&wrong_authority, |_| Ok::<_, Infallible>(vec![])),
        Err(RetainError::State(Error::InvalidBinding))
    ));
    let wrong_message = Delivery::new(
        2.into(),
        3.into(),
        Message::<String, String, String>::SubmitOperation(Operation {
            operation_id: 10.into(),
            task: "task".to_owned(),
        }),
    );
    assert!(matches!(
        outbox.retain(&wrong_message, |_| Ok::<_, Infallible>(vec![])),
        Err(RetainError::State(Error::WrongMessage))
    ));
    assert!(matches!(
        outbox.retain(&dispatch(11, 21), |_| Ok::<_, Infallible>(vec![])),
        Err(RetainError::State(Error::Capacity))
    ));
    outbox.acknowledge(binding()).unwrap();
    // Full-definition conflict checks survive release of encoded bytes.
    let mut changed = original.clone();
    let Message::DispatchAttempt(attempt) = &mut changed.message else {
        unreachable!()
    };
    attempt.operation.task = "after confirmation".to_owned();
    assert!(matches!(
        outbox.retain(&changed, |_| Ok::<_, Infallible>(vec![])),
        Err(RetainError::State(Error::Conflict))
    ));
    retain(&mut outbox, &dispatch(11, 21));
    assert!(!outbox.has_capacity());
}

#[tokio::test(start_paused = true)]
async fn only_a_report_retained_by_the_original_authority_stops_redelivery() {
    let mut authority =
        Authority::<String, String, String>::new(2.into(), vec![3.into()], LedgerLimits::default())
            .unwrap();
    let submitted = authority
        .submit_keyed(7.into(), "task".to_owned(), || (10.into(), 20.into()))
        .unwrap();
    let mut outbox = outbox(Limits::default());
    retain(&mut outbox, &submitted.dispatch);
    let ticket = next(&mut outbox).ticket();
    outbox.sent(ticket);
    assert_eq!(
        outbox.reported(&authority, 30.into()),
        Err(Error::ReportNotRetained)
    );
    let Message::DispatchAttempt(attempt) = &submitted.dispatch.message else {
        panic!("expected dispatch")
    };
    let mut invalid = report(attempt.clone());
    invalid.attempt_id = 99.into();
    assert!(
        authority
            .receive_report(Delivery::new(
                3.into(),
                2.into(),
                Message::SubmitReport(invalid)
            ))
            .is_err()
    );
    assert_eq!(
        outbox.reported(&authority, 30.into()),
        Err(Error::ReportNotRetained)
    );
    assert_eq!(outbox.progress(10.into()).unwrap().status, Status::Active);
    let mut other =
        Authority::<String, String, String>::new(9.into(), vec![3.into()], LedgerLimits::default())
            .unwrap();
    other
        .submit_keyed(7.into(), "task".to_owned(), || (10.into(), 20.into()))
        .unwrap();
    other
        .receive_report(Delivery::new(
            3.into(),
            9.into(),
            Message::SubmitReport(report(attempt.clone())),
        ))
        .unwrap();
    assert_eq!(
        outbox.reported(&other, 30.into()),
        Err(Error::InvalidBinding)
    );
    authority
        .receive_report(Delivery::new(
            3.into(),
            2.into(),
            Message::SubmitReport(report(attempt.clone())),
        ))
        .unwrap();
    assert!(outbox.reported(&authority, 30.into()).unwrap());
    assert!(
        !outbox.acknowledge(binding()).unwrap(),
        "receipt arriving after evidence is a duplicate confirmation"
    );
    advance(Duration::from_secs(10)).await;
    assert!(outbox.next_event().is_none());
}

#[tokio::test(start_paused = true)]
async fn lost_receipt_reuses_one_worker_execution_and_cached_report() {
    let mut worker =
        Worker::<String, String, String>::new(2.into(), vec![3.into()], LedgerLimits::default())
            .unwrap();
    let mut outbox = outbox(Limits::default());
    let original = dispatch(10, 20);
    retain(&mut outbox, &original);
    let first = next(&mut outbox);
    let ticket = first.ticket();
    first
        .send(|_, _| async {
            assert!(worker.receive_dispatch(original.clone()).unwrap().is_none());
            Ok::<_, io::Error>(())
        })
        .await
        .unwrap();
    outbox.sent(ticket);
    let first_report = worker
        .next_report(|attempt| async move { report(attempt) })
        .await
        .unwrap()
        .unwrap();
    advance(Duration::from_secs(1)).await;
    let second = next(&mut outbox);
    let ticket = second.ticket();
    second
        .send(|_, _| async {
            assert_eq!(
                worker.receive_dispatch(original.clone()).unwrap(),
                Some(first_report)
            );
            Ok::<_, io::Error>(())
        })
        .await
        .unwrap();
    outbox.acknowledge(binding()).unwrap();
    outbox.sent(ticket);
    assert!(
        worker
            .next_report(|_| async { panic!("duplicate delivery must never rerun the executor") })
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(outbox.progress(10.into()).unwrap().sends, 2);
    assert!(outbox.next_event().is_none());
}

#[tokio::test(start_paused = true)]
async fn cancelling_send_stops_its_future_and_keeps_the_original_budget() {
    let mut outbox = outbox(Limits::default());
    retain(&mut outbox, &dispatch(10, 20));
    let transmission = next(&mut outbox);
    let ticket = transmission.ticket();
    let completed = Cell::new(false);
    let result = tokio::time::timeout(
        Duration::from_millis(100),
        transmission.send(|_, _| async {
            tokio::time::sleep(Duration::from_secs(1)).await;
            completed.set(true);
            Ok::<_, io::Error>(())
        }),
    )
    .await;
    assert!(result.is_err());
    assert!(outbox.sent(ticket));
    advance(Duration::from_secs(1)).await;
    assert!(
        !completed.get(),
        "cancelled send must not survive in a background task"
    );
    assert_eq!(next(&mut outbox).ticket().number(), 2);
}
