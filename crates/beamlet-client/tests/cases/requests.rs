use crate::support;
use crate::support::{Event, instance};
use beamlet_api::{
    Command, Outcome, Pool, Reply, Selection, Submission, Task, TransportError, Uuid,
};
use beamlet_client::{Client, Error, Options};
use std::time::Duration;

fn submission<P>(payload: P) -> Submission<P> {
    Submission {
        pool: Pool::default(),
        worker: Selection::Exact(instance(3)),
        task: Task {
            capability: "echo/v1".into(),
            payload,
        },
    }
}
fn completed<R>(result: R) -> Outcome<R> {
    Outcome::Completed {
        operation_id: "op".into(),
        attempt_id: "attempt".into(),
        report_id: "report".into(),
        worker: instance(3),
        result,
    }
}
// 故意不实现 Clone、Debug、Serialize; Cell 同时证明无需 Sync.
struct Input(std::cell::Cell<usize>);
struct Output(usize);
#[tokio::test]
async fn concurrent_calls_move_custom_data_and_keep_replies_without_hidden_capacity() {
    tokio::time::timeout(Duration::from_secs(3), async {
        let (transport, mut events) = support::pair::<Input, Output>();
        let client = Client::new(transport, Options::default()).unwrap();
        let mut calls = Vec::new();
        for index in 0..24 {
            let client = client.clone();
            calls.push((
                index,
                tokio::spawn(async move {
                    client
                        .submit(instance(1), submission(Input(index.into())))
                        .await
                }),
            ));
        }
        let mut pending = Vec::new();
        let mut ids = std::collections::BTreeSet::new();
        for _ in 0..calls.len() {
            let Event::Request {
                command,
                deadline,
                reply,
            } = events.recv().await.unwrap()
            else {
                panic!("expected request")
            };
            assert_eq!(deadline, Options::default().request_timeout);
            let Command::Submit {
                request_id,
                authority,
                submission,
            } = command
            else {
                panic!("expected submit")
            };
            assert!(ids.insert(request_id));
            assert_eq!(authority, instance(1));
            pending.push((request_id, submission.task.payload.0.get(), reply));
        }
        for (request_id, index, reply) in pending.into_iter().rev() {
            assert!(
                reply
                    .send(Ok(Reply {
                        request_id,
                        authority: instance(1),
                        outcome: completed(Output(index))
                    }))
                    .is_ok()
            );
        }
        for (index, call) in calls {
            let Outcome::Completed { result, .. } = call.await.unwrap().unwrap() else {
                panic!("expected completion")
            };
            assert_eq!(result.0, index);
        }
    })
    .await
    .unwrap();
}
#[tokio::test(start_paused = true)]
async fn correlation_kind_and_exact_worker_mismatches_are_errors_without_retry() {
    for case in 0..7 {
        let (transport, mut events) = support::pair::<String, String>();
        let client = Client::new(transport, Options::default()).unwrap();
        let id = Uuid::new_v4();
        let call = tokio::spawn(async move {
            client
                .request(Command::Submit {
                    request_id: id,
                    authority: instance(1),
                    submission: submission("payload".to_owned()),
                })
                .await
        });
        let Event::Request { command, reply, .. } = events.recv().await.unwrap() else {
            panic!("expected request")
        };
        assert_eq!(command.request_id(), id);
        let mut response = Reply {
            request_id: id,
            authority: instance(1),
            outcome: completed("ok".to_owned()),
        };
        match case {
            0 => response.request_id = Uuid::new_v4(),
            1 => response.authority.session_id = Uuid::new_v4(),
            2 => response.outcome = Outcome::Workers { workers: vec![] },
            3 => {
                if let Outcome::Completed { worker, .. } = &mut response.outcome {
                    worker.session_id = Uuid::new_v4();
                }
            }
            4 => {
                if let Outcome::Completed { report_id, .. } = &mut response.outcome {
                    report_id.clear();
                }
            }
            5 => {
                response.outcome = Outcome::Rejected {
                    reason: "full".into(),
                }
            }
            6 => {
                response.outcome = Outcome::Unresolved {
                    operation_id: "op".into(),
                    attempt_id: "attempt".into(),
                    worker: instance(3),
                    reason: "no report".into(),
                }
            }
            _ => unreachable!(),
        }
        reply.send(Ok(response)).unwrap();
        match (case, call.await.unwrap()) {
            (0..=4, Err(Error::InvalidReply))
            | (5, Ok(Outcome::Rejected { .. }))
            | (6, Ok(Outcome::Unresolved { .. })) => {}
            (_, result) => panic!("case {case}: {result:?}"),
        }
        assert!(events.try_recv().is_err(), "unexpected retry");
    }
}
#[tokio::test(start_paused = true)]
async fn timeout_cancellation_and_transport_errors_release_waiting_without_retry() {
    for case in 0..3 {
        let (transport, mut events) = support::pair::<String, String>();
        let client = Client::new(transport, Options::default()).unwrap();
        let call = tokio::spawn(async move {
            client
                .submit(instance(1), submission("payload".to_owned()))
                .await
        });
        let Event::Request { reply, .. } = events.recv().await.unwrap() else {
            panic!("expected request")
        };
        match case {
            0 => {
                assert!(matches!(call.await.unwrap(), Err(Error::Timeout)));
                assert!(reply.is_closed());
            }
            1 => {
                call.abort();
                assert!(call.await.unwrap_err().is_cancelled());
                assert!(reply.is_closed());
            }
            _ => {
                reply
                    .send(Err(TransportError::new(std::io::Error::other(
                        "disconnected",
                    ))))
                    .unwrap();
                let error = call.await.unwrap().unwrap_err();
                assert!(matches!(error, Error::Transport(_)));
                assert!(
                    std::error::Error::source(&error)
                        .unwrap()
                        .to_string()
                        .contains("disconnected")
                );
            }
        }
        assert!(events.try_recv().is_err(), "unexpected retry");
    }
}
#[tokio::test]
async fn worker_query_rejection_and_wrong_result_kind_are_typed_errors() {
    for outcome in [
        Outcome::<String>::Rejected {
            reason: "wrong authority".into(),
        },
        completed("bad".into()),
        Outcome::Workers { workers: vec![] },
    ] {
        let (transport, mut events) = support::pair::<String, String>();
        let client = Client::new(transport, Options::default()).unwrap();
        let rejected = matches!(outcome, Outcome::Rejected { .. });
        let completed = matches!(outcome, Outcome::Completed { .. });
        let call = tokio::spawn(async move {
            client
                .workers(instance(1), Pool::default(), Some("echo/v1"))
                .await
        });
        let Event::Request { command, reply, .. } = events.recv().await.unwrap() else {
            panic!("expected query")
        };
        assert!(
            matches!(&command, Command::Workers { capability: Some(cap), .. } if cap == "echo/v1")
        );
        reply
            .send(Ok(Reply {
                request_id: command.request_id(),
                authority: instance(1),
                outcome,
            }))
            .unwrap();
        match (rejected, completed, call.await.unwrap()) {
            (true, _, Err(Error::Rejected { .. })) | (_, true, Err(Error::InvalidReply)) => {}
            (false, false, Ok(workers)) => assert!(workers.is_empty()),
            (_, _, result) => panic!("unexpected query result: {result:?}"),
        }
    }
}

#[tokio::test]
async fn worker_snapshots_must_match_pool_and_filter_but_can_be_stale() {
    use beamlet_api::{Presence, PresenceState, WorkerView};
    for case in 0..4 {
        let (transport, mut events) = support::pair::<String, String>();
        let client = Client::new(transport, Options::default()).unwrap();
        let call = tokio::spawn(async move {
            client
                .workers(instance(1), Pool::default(), Some("echo/v1"))
                .await
        });
        let Event::Request { command, reply, .. } = events.recv().await.unwrap() else {
            panic!("expected query");
        };
        let mut view = WorkerView {
            presence: Presence {
                pool: Pool::default(),
                instance: instance(3),
                label: None,
                capabilities: ["echo/v1".into()].into(),
                sequence: 1,
                state: PresenceState::Offline,
            },
            last_seen_ms: 30_000,
            stale: true,
        };
        match case {
            0 => view.presence.pool = "other".parse().unwrap(),
            1 => view.presence.capabilities.clear(),
            2 => view.presence.instance.session_id = Uuid::nil(),
            _ => {}
        }
        reply
            .send(Ok(Reply {
                request_id: command.request_id(),
                authority: instance(1),
                outcome: Outcome::Workers {
                    workers: vec![view],
                },
            }))
            .unwrap();
        match (case, call.await.unwrap()) {
            (0..=2, Err(Error::InvalidReply)) => {}
            (3, Ok(workers)) => assert!(workers[0].stale),
            (_, result) => panic!("case {case}: {result:?}"),
        }
    }
}
