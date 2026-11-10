use beamlet::{
    ExecutionBinding,
    model::{Delivery, ExecutionResult, ImpactStatus, Message, Operation, SubmitReport},
    report_delivery::{
        Binding, Error, Event, Limits, Outbox, RetainError, SendError, Status, Transmission,
    },
};
use std::{cell::Cell, convert::Infallible, io, time::Duration};
use tokio::time::{Instant, advance};

type Report = Delivery<(), String, String>;

fn report(id: u128) -> Report {
    Delivery::new(
        3.into(),
        2.into(),
        Message::SubmitReport(SubmitReport {
            report_id: id.into(),
            operation_id: 10.into(),
            attempt_id: 20.into(),
            impact: ImpactStatus::Unknown,
            result: ExecutionResult::Failed {
                error: "original error".to_owned(),
            },
        }),
    )
}
fn evidence(report: &mut Report) -> &mut SubmitReport<String, String> {
    let Message::SubmitReport(evidence) = &mut report.message else {
        panic!("expected report")
    };
    evidence
}
fn binding(id: u128) -> Binding {
    Binding {
        execution: ExecutionBinding {
            authority_instance_id: 2.into(),
            worker_instance_id: 3.into(),
            operation_id: 10.into(),
            attempt_id: 20.into(),
        },
        report_id: id.into(),
    }
}
fn outbox(limits: Limits) -> Outbox<String, String> {
    Outbox::new(3.into(), 2.into(), limits).unwrap()
}
fn retain(outbox: &mut Outbox<String, String>, report: &Report) {
    outbox
        .retain(report, |_| Ok::<_, Infallible>(b"original bytes".to_vec()))
        .unwrap();
}
fn next(outbox: &mut Outbox<String, String>) -> Transmission {
    let Some(Event::Send(transmission)) = outbox.next_event() else {
        panic!("expected transmission")
    };
    transmission
}

#[tokio::test(start_paused = true)]
async fn retries_preserve_bytes_and_budget_for_success_error_and_timeout() {
    for mode in 0..3 {
        let mut outbox = outbox(Limits::default());
        let encodings = Cell::new(0);
        outbox
            .retain(&report(30), |_| {
                encodings.set(encodings.get() + 1);
                Ok::<_, Infallible>(b"original bytes".to_vec())
            })
            .unwrap();
        let start = Instant::now();
        for number in 1..=3 {
            let transmission = next(&mut outbox);
            let ticket = transmission.ticket();
            assert_eq!(ticket.binding(), binding(30));
            assert_eq!(ticket.number(), number);
            assert!(
                outbox.next_event().is_none(),
                "only one in-flight transmission per report"
            );
            let result = transmission
                .send(|authority, bytes| async move {
                    assert_eq!(authority, 2.into());
                    assert_eq!(bytes, b"original bytes");
                    match mode {
                        0 => Ok(()),
                        1 => Err(io::Error::other("offline")),
                        _ => std::future::pending().await,
                    }
                })
                .await;
            match mode {
                0 => assert!(result.is_ok()),
                1 => assert!(matches!(result, Err(SendError::Transport(_)))),
                _ => assert!(matches!(result, Err(SendError::TimedOut))),
            }
            assert_eq!(
                outbox.unconfirmed(),
                1,
                "transport completion is not retention"
            );
            assert!(outbox.sent(ticket));
            advance(Duration::from_millis(999)).await;
            assert!(
                !outbox.sent(ticket),
                "duplicate completion cannot reset the retry delay"
            );
            assert!(outbox.next_event().is_none());
            outbox
                .retain(&report(30), |_| -> Result<Vec<u8>, Infallible> {
                    panic!("duplicates must not encode or refresh the budget")
                })
                .unwrap();
            advance(Duration::from_millis(1)).await;
        }
        let Some(Event::Exhausted(progress)) = outbox.next_event() else {
            panic!("expected exhausted budget")
        };
        assert_eq!(progress.sends, 3);
        assert_eq!(progress.status, Status::Unconfirmed);
        assert_eq!(
            start.elapsed(),
            Duration::from_millis(if mode == 2 { 4500 } else { 3000 })
        );
        assert_eq!(encodings.get(), 1);
        retain(&mut outbox, &report(30));
        assert!(outbox.next_event().is_none());
        assert_eq!(
            outbox.unconfirmed(),
            1,
            "exhaustion must not permit clean draining"
        );
        assert!(outbox.acknowledge(binding(30)).unwrap());
        assert!(!outbox.acknowledge(binding(30)).unwrap());
        assert_eq!(outbox.unconfirmed(), 0);
        assert!(outbox.next_event().is_none());
    }
}

#[tokio::test(start_paused = true)]
async fn total_deadline_expires_held_transmissions_but_preserves_unconfirmed_evidence() {
    let mut outbox = outbox(Limits {
        max_records: 2,
        max_pending: 1,
    });
    retain(&mut outbox, &report(30));
    let transmission = next(&mut outbox);
    let ticket = transmission.ticket();
    assert!(!outbox.has_capacity(0));
    advance(Duration::from_millis(4999)).await;
    assert!(outbox.next_event().is_none());
    retain(&mut outbox, &report(30));
    advance(Duration::from_millis(1)).await;
    let Some(Event::Exhausted(progress)) = outbox.next_event() else {
        panic!("original five-second deadline must win")
    };
    assert_eq!(progress.sends, 1);
    assert_eq!(outbox.unconfirmed(), 1);
    assert!(outbox.has_capacity(0));
    drop(transmission);
    assert!(outbox.sent(ticket));
    assert!(outbox.next_event().is_none());
    retain(&mut outbox, &report(31));
    assert_eq!(outbox.unconfirmed(), 2);
    assert!(outbox.acknowledge(binding(30)).unwrap());
    assert_eq!(outbox.unconfirmed(), 1);
    assert!(
        !outbox.has_capacity(0),
        "dedup records are not evicted to create new budgets"
    );
}

#[tokio::test(start_paused = true)]
async fn full_receipt_binding_and_send_sequence_prevent_cross_confirmation() {
    let mut outbox = outbox(Limits::default());
    retain(&mut outbox, &report(30));
    retain(&mut outbox, &report(31));
    for field in 0..5 {
        let mut wrong = binding(30);
        match field {
            0 => wrong.execution.authority_instance_id = 99.into(),
            1 => wrong.execution.worker_instance_id = 99.into(),
            2 => wrong.execution.operation_id = 99.into(),
            3 => wrong.execution.attempt_id = 99.into(),
            _ => wrong.report_id = 99.into(),
        }
        assert!(outbox.acknowledge(wrong).is_err());
        assert_eq!(outbox.unconfirmed(), 2);
    }
    // Independent evidence for the same Attempt must still need its own receipt.
    assert!(outbox.acknowledge(binding(31)).unwrap());
    assert_eq!(outbox.progress(30.into()).unwrap().status, Status::Active);
    let first = next(&mut outbox).ticket();
    assert!(outbox.sent(first));
    advance(Duration::from_secs(1)).await;
    let second = next(&mut outbox).ticket();
    assert!(
        !outbox.sent(first),
        "old completion must not unlock the new transmission"
    );
    advance(Duration::from_secs(1)).await;
    assert!(outbox.next_event().is_none());
    assert!(outbox.acknowledge(binding(30)).unwrap());
    assert!(outbox.sent(second));
    advance(Duration::from_secs(10)).await;
    assert!(outbox.next_event().is_none());
    retain(&mut outbox, &report(30));
    assert!(outbox.next_event().is_none());
    assert_eq!(outbox.progress(30.into()).unwrap().sends, 2);
    assert_eq!(outbox.unconfirmed(), 0);
}

#[test]
fn retain_rejects_wrong_messages_instances_and_conflicting_complete_evidence() {
    let mut outbox = outbox(Limits::default());
    let original = report(30);
    retain(&mut outbox, &original);
    for field in 0..6 {
        let mut changed = original.clone();
        match field {
            0 => changed.source_instance_id = 99.into(),
            1 => changed.target_instance_id = 99.into(),
            2 => evidence(&mut changed).operation_id = 99.into(),
            3 => evidence(&mut changed).attempt_id = 99.into(),
            4 => evidence(&mut changed).impact = ImpactStatus::Observed,
            _ => {
                evidence(&mut changed).result = ExecutionResult::Succeeded {
                    output: "replacement".to_owned(),
                }
            }
        }
        assert!(
            outbox
                .retain(&changed, |_| -> Result<Vec<u8>, Infallible> {
                    panic!("a conflict cannot call the encoder")
                })
                .is_err()
        );
        assert_eq!(outbox.progress(30.into()).unwrap().sends, 0);
    }
    let wrong = Delivery::new(
        3.into(),
        2.into(),
        Message::SubmitOperation(Operation {
            operation_id: 10.into(),
            task: (),
        }),
    );
    assert!(matches!(
        outbox.retain(&wrong, |_| Ok::<_, Infallible>(vec![])),
        Err(RetainError::State(Error::WrongMessage))
    ));
    let mut invalid = report(31);
    evidence(&mut invalid).result = ExecutionResult::Rejected {
        error: "not started".to_owned(),
    };
    assert!(matches!(
        outbox.retain(&invalid, |_| Ok::<_, Infallible>(vec![])),
        Err(RetainError::State(Error::InvalidEvidence(_)))
    ));
    assert!(outbox.progress(31.into()).is_none());
    assert!(outbox.acknowledge(binding(30)).unwrap());
    let mut replacement = original.clone();
    evidence(&mut replacement).result = ExecutionResult::Failed {
        error: "replacement error".to_owned(),
    };
    assert!(matches!(
        outbox.retain(&replacement, |_| Ok::<_, Infallible>(vec![])),
        Err(RetainError::State(Error::Conflict))
    ));
    retain(&mut outbox, &original);
    assert_eq!(
        outbox.progress(30.into()).unwrap().status,
        Status::Confirmed
    );
}

#[test]
fn pending_execution_reserves_both_record_and_active_capacity_without_encoding_on_failure() {
    for limits in [
        Limits {
            max_records: 0,
            max_pending: 1,
        },
        Limits {
            max_records: 1,
            max_pending: 0,
        },
    ] {
        assert!(matches!(
            Outbox::<String, String>::new(3.into(), 2.into(), limits),
            Err(Error::InvalidLimits)
        ));
    }
    for limits in [
        Limits {
            max_records: 2,
            max_pending: 3,
        },
        Limits {
            max_records: 3,
            max_pending: 2,
        },
    ] {
        let mut outbox = outbox(limits);
        assert!(outbox.has_capacity(1));
        assert!(!outbox.has_capacity(2));
        assert!(!outbox.has_capacity(usize::MAX));
        assert!(matches!(
            outbox.retain(&report(30), |_| Err(io::Error::other("encoding failed"))),
            Err(RetainError::Encoding(_))
        ));
        assert_eq!(outbox.unconfirmed(), 0);
        assert!(outbox.progress(30.into()).is_none());
        retain(&mut outbox, &report(30));
        assert!(!outbox.has_capacity(1));
        retain(&mut outbox, &report(31));
        assert!(matches!(
            outbox.retain(&report(32), |_| -> Result<Vec<u8>, Infallible> {
                panic!("no capacity to encode")
            }),
            Err(RetainError::State(Error::Capacity))
        ));
        retain(&mut outbox, &report(30));
        assert!(outbox.acknowledge(binding(30)).unwrap());
        assert_eq!(outbox.has_capacity(0), limits.max_records == 3);
        assert_eq!(outbox.unconfirmed(), 1);
    }
}

#[tokio::test(start_paused = true)]
async fn cancellation_stops_the_send_future_without_renewing_the_original_budget() {
    struct OnDrop<'a>(&'a Cell<bool>);
    impl Drop for OnDrop<'_> {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    let mut outbox = outbox(Limits::default());
    retain(&mut outbox, &report(30));
    let transmission = next(&mut outbox);
    let ticket = transmission.ticket();
    let dropped = Cell::new(false);
    {
        let send = transmission.send(|_, _| async {
            let _guard = OnDrop(&dropped);
            std::future::pending::<Result<(), Infallible>>().await
        });
        tokio::pin!(send);
        tokio::select! {
            biased;
            _ = &mut send => panic!("pending transport cannot complete"),
            _ = tokio::time::sleep(Duration::from_millis(100)) => {}
        }
    }
    assert!(dropped.get());
    assert!(outbox.sent(ticket));
    advance(Duration::from_secs(1)).await;
    assert_eq!(next(&mut outbox).ticket().number(), 2);
    assert_eq!(outbox.unconfirmed(), 1);
}
