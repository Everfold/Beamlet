use super::*;
use beamlet_protocol::Presence;
use std::time::Duration;
use tokio::sync::oneshot;

async fn within<F: Future>(future: F) -> F::Output {
    tokio::time::timeout(Duration::from_secs(5), future)
        .await
        .expect("console async test timed out")
}
fn instance(id: u128) -> RoleInstance {
    RoleInstance {
        role_id: Uuid::from_u128(id),
        session_id: Uuid::from_u128(id + 100),
    }
}
fn authorities() -> Vec<AuthorityInfo> {
    [1, 2]
        .into_iter()
        .map(|id| AuthorityInfo {
            instance: instance(id),
            label: None,
            pools: BTreeSet::from([Pool::default()]),
        })
        .collect()
}
fn workers() -> Vec<WorkerView> {
    vec![WorkerView {
        presence: Presence {
            pool: Pool::default(),
            instance: instance(3),
            label: None,
            capabilities: BTreeSet::from(["text.reverse/v1".into()]),
            sequence: 1,
            state: PresenceState::Ready,
        },
        last_seen_ms: 0,
        stale: false,
    }]
}
fn selected() -> Session {
    let mut session = Session::new(
        BTreeSet::from([Pool::default()]),
        Settings::default().max_in_flight_requests,
    );
    session.discovered(authorities());
    session.line("/authority 1".into()).unwrap();
    session.line("/pool default".into()).unwrap();
    session.update_workers(workers());
    session.line("/capability 1".into()).unwrap();
    session.line("/worker 1".into()).unwrap();
    session
}
fn response(action: Action, result: Result<Outcome>) -> Completion {
    let Action::Request {
        command,
        choice_revision,
    } = action
    else {
        panic!("expected request");
    };
    Completion::reply(command, choice_revision, result)
}
fn completed(text: &str) -> Outcome {
    Outcome::Completed {
        operation_id: text.into(),
        attempt_id: format!("{text}:attempt"),
        report_id: format!("{text}:report"),
        worker: instance(3),
        result: text.into(),
    }
}
fn track_query(session: &mut Session) -> Action {
    let query = session.workers().unwrap();
    session.workers_request = Some(query.request_id());
    query
}

#[test]
fn submission_keeps_target_snapshot_and_late_rejection_preserves_new_choice() {
    let mut session = selected();
    let action = session.line("first".into()).unwrap().unwrap();
    let Action::Request {
        command:
            Command::Submit {
                authority,
                submission,
                ..
            },
        ..
    } = &action
    else {
        panic!("expected submission");
    };
    assert_eq!(*authority, instance(1));
    assert_eq!(submission.worker, Selection::Exact(instance(3)));
    assert_eq!(submission.task.text, "first");
    // 即使重新选择同一个 Worker, 旧拒绝也不能撤销这次明确选择.
    session.line("/worker 1".into()).unwrap();
    session
        .complete(response(
            action,
            Ok(Outcome::Rejected {
                reason: "old rejection".into(),
            }),
        ))
        .unwrap();
    assert_eq!(session.choice.worker, Some(Selection::Exact(instance(3))));
    let current = session.line("second".into()).unwrap().unwrap();
    session
        .complete(response(
            current,
            Ok(Outcome::Rejected {
                reason: "current rejection".into(),
            }),
        ))
        .unwrap();
    assert_eq!(session.choice.worker, None);
}

#[test]
fn older_queries_and_discovery_never_replace_newer_results() {
    let mut session = selected();
    let old = track_query(&mut session);
    let new = track_query(&mut session);
    session
        .complete(response(new, Ok(Outcome::Workers { workers: workers() })))
        .unwrap();
    session
        .complete(response(old, Ok(Outcome::Workers { workers: vec![] })))
        .unwrap();
    assert_eq!(session.workers.len(), 1);
    let before_selection = track_query(&mut session);
    session.line("/worker 1".into()).unwrap();
    session
        .complete(response(
            before_selection,
            Ok(Outcome::Workers { workers: vec![] }),
        ))
        .unwrap();
    assert_eq!(session.choice.worker, Some(Selection::Exact(instance(3))));
    assert_eq!(session.workers.len(), 1);
    let old_target = track_query(&mut session);
    session.line("/authority 2".into()).unwrap();
    session
        .complete(response(
            old_target,
            Ok(Outcome::Workers { workers: workers() }),
        ))
        .unwrap();
    assert_eq!(session.choice.authority, Some(instance(2)));
    assert!(session.workers.is_empty());

    let old = Uuid::new_v4();
    let new = Uuid::new_v4();
    session.discovery_request = Some(new);
    session
        .complete(Completion::Discovered {
            request_id: new,
            result: Ok(authorities()),
        })
        .unwrap();
    session
        .complete(Completion::Discovered {
            request_id: old,
            result: Ok(vec![]),
        })
        .unwrap();
    assert_eq!(session.authorities.len(), 2);
    assert_eq!(session.choice.authority, Some(instance(2)));
}

#[tokio::test]
async fn pending_submissions_allow_queries_and_eof_drains_out_of_order_results() {
    within(async {
        let mut session = selected();
        let (input, receive) = mpsc::channel(8);
        let (events, mut pending) = mpsc::unbounded_channel();
        let running = tokio::spawn(async move {
            drive(&mut session, receive, move |action: Action| {
                let events = events.clone();
                async move {
                    let (send, receive) = oneshot::channel();
                    events
                        .send((action, send))
                        .unwrap_or_else(|_| panic!("test observer closed"));
                    receive.await.unwrap()
                }
            })
            .await
            .unwrap();
            session
        });
        let (discovery, reply) = pending.recv().await.unwrap();
        reply
            .send(Completion::Discovered {
                request_id: discovery.request_id(),
                result: Ok(authorities()),
            })
            .unwrap_or_else(|_| panic!("discovery receiver closed"));
        input.send(Ok("slow".into())).await.unwrap();
        input.send(Ok("fast".into())).await.unwrap();
        input.send(Ok("/workers".into())).await.unwrap();
        let mut submissions = BTreeSet::new();
        let mut held = Vec::new();
        for _ in 0..3 {
            let (action, reply) = pending.recv().await.unwrap();
            if let Action::Request {
                command: Command::Submit { submission, .. },
                ..
            } = &action
            {
                submissions.insert(submission.task.text.clone());
            }
            held.push((action, reply));
        }
        assert_eq!(submissions, BTreeSet::from(["slow".into(), "fast".into()]));
        drop(input); // EOF 不丢弃在途请求.
        assert!(!running.is_finished());
        for (action, reply) in held.into_iter().rev() {
            let outcome = match &action {
                Action::Request {
                    command: Command::Workers { .. },
                    ..
                } => Outcome::Workers { workers: workers() },
                Action::Request {
                    command: Command::Submit { submission, .. },
                    ..
                } => completed(&submission.task.text),
                _ => panic!("unexpected discovery"),
            };
            reply
                .send(response(action, Ok(outcome)))
                .unwrap_or_else(|_| panic!("request receiver closed"));
        }
        let session = running.await.unwrap();
        assert_eq!(session.choice.worker, Some(Selection::Exact(instance(3))));
        assert!(
            pending.recv().await.is_none(),
            "requests must not be retried"
        );
    })
    .await;
}

#[tokio::test]
async fn full_console_still_accepts_quit_and_does_not_send_excess_requests() {
    for limit in [1, 3, 32] {
        within(async {
            let mut session = selected();
            session.max_in_flight_requests = limit;
            let (input, receive) = mpsc::channel(limit + 4);
            let (events, mut pending) = mpsc::unbounded_channel();
            let running = tokio::spawn(async move {
                drive(&mut session, receive, move |action: Action| {
                    let events = events.clone();
                    async move {
                        let (send, receive) = oneshot::channel();
                        events
                            .send((action, send))
                            .unwrap_or_else(|_| panic!("test observer closed"));
                        receive.await.unwrap()
                    }
                })
                .await
            });
            let (discovery, reply) = pending.recv().await.unwrap();
            // 发现占一个名额; 其余名额全部占住, 不靠定时等待猜测任务是否已完成.
            let mut held = vec![(discovery, reply)];
            for i in 1..limit {
                input.send(Ok(format!("text-{i}"))).await.unwrap();
                held.push(pending.recv().await.unwrap());
            }
            input.send(Ok("overflow".into())).await.unwrap();
            input.send(Ok("/help".into())).await.unwrap();
            input.send(Ok("/quit".into())).await.unwrap();
            input.closed().await;
            assert!(!running.is_finished(), "quit must drain pending requests");
            assert!(
                pending.try_recv().is_err(),
                "full console sent an excess request"
            );
            for (action, reply) in held.into_iter().rev() {
                let completion = match action {
                    Action::Discover { request_id, .. } => Completion::Discovered {
                        request_id,
                        result: Ok(authorities()),
                    },
                    action => response(action, Err(anyhow::anyhow!("simulated request timeout"))),
                };
                reply
                    .send(completion)
                    .unwrap_or_else(|_| panic!("request receiver closed"));
            }
            running.await.unwrap().unwrap();
            assert!(
                pending.recv().await.is_none(),
                "failed requests must not be retried"
            );
        })
        .await;
    }
}

#[tokio::test]
async fn interrupting_console_drops_pending_futures() {
    struct DropNotice(Option<oneshot::Sender<()>>);
    impl Drop for DropNotice {
        fn drop(&mut self) {
            if let Some(send) = self.0.take() {
                let _ = send.send(());
            }
        }
    }
    within(async {
        let mut session = selected();
        let (input, receive) = mpsc::channel(1);
        let (events, mut started) = mpsc::unbounded_channel();
        let running = tokio::spawn(async move {
            drive(&mut session, receive, move |_| {
                let events = events.clone();
                async move {
                    let (send, receive) = oneshot::channel();
                    let _drop = DropNotice(Some(send));
                    events.send(receive).unwrap();
                    std::future::pending().await
                }
            })
            .await
        });
        let discovery = started.recv().await.unwrap();
        input.send(Ok("pending".into())).await.unwrap();
        let submission = started.recv().await.unwrap();
        running.abort();
        assert!(running.await.unwrap_err().is_cancelled());
        discovery.await.unwrap();
        submission.await.unwrap();
        assert!(started.recv().await.is_none());
    })
    .await;
}
