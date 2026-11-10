use beamlet::{
    Authority, OperationObservation,
    memory::Limits,
    model::{Delivery, ExecutionResult, ImpactStatus, Message, SubmitReport},
    observe::{self, Budget, Change, MergeError, Notice, State},
    query::{self, Observation, Reply, Request, Selection},
    remote::{Client, Transport},
    state::{OperationPhase, WaitStatus},
};
use std::{
    io,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    sync::mpsc,
    time::{Instant, sleep},
};

type Fact = Notice<String, String, String>;
type View = OperationObservation<String>;
type Answer = Reply<String, String, String>;
type Incoming = Result<Option<Vec<u8>>, io::Error>;

fn facts() -> (View, View, Fact, Fact) {
    let mut authority = Authority::new(2.into(), vec![3.into()], Limits::default()).unwrap();
    authority
        .submit_keyed(7.into(), "task".to_owned(), || (10.into(), 20.into()))
        .unwrap();
    let request = Request {
        id: 1.into(),
        authority_instance_id: 2.into(),
        selection: Selection::Operation(10.into()),
    };
    let Observation::Operation(accepted) = authority.answer_query(request).unwrap().observation
    else {
        panic!("expected operation")
    };
    authority.mark_dispatched(10.into()).unwrap();
    authority.mark_wait_timeout(10.into()).unwrap();
    let Observation::Operation(pending) = authority.answer_query(request).unwrap().observation
    else {
        panic!("expected operation")
    };
    for id in [30, 31] {
        authority
            .receive_report(Delivery::new(
                3.into(),
                2.into(),
                Message::SubmitReport(SubmitReport {
                    report_id: id.into(),
                    operation_id: 10.into(),
                    attempt_id: 20.into(),
                    impact: ImpactStatus::Unknown,
                    result: ExecutionResult::Failed {
                        error: format!("evidence {id}"),
                    },
                }),
            ))
            .unwrap();
    }
    (
        accepted,
        pending,
        authority.report_notice(30.into()).unwrap(),
        authority.report_notice(31.into()).unwrap(),
    )
}

#[test]
fn every_snapshot_and_notice_order_converges_and_delivers_only_the_adopted_report_once() {
    let (accepted, pending, adopted, additional) = facts();
    for a in 0..4 {
        for b in 0..4 {
            for c in 0..4 {
                for d in 0..4 {
                    let order = [a, b, c, d];
                    if (0..4).any(|i| order.iter().filter(|value| **value == i).count() != 1) {
                        continue;
                    }
                    let mut state = State::new(2.into(), 10.into(), 2).unwrap();
                    let mut reports = Vec::new();
                    for index in order {
                        let change = match index {
                            0 => state.merge_operation(accepted.clone()),
                            1 => state.merge_operation(pending.clone()),
                            2 => state.merge_notice(adopted.clone()),
                            _ => state.merge_notice(additional.clone()),
                        }
                        .unwrap();
                        if let Some(id) = change.new_report {
                            reports.push(id);
                        }
                    }
                    assert_eq!(
                        state.operation(),
                        Some(&adopted.operation),
                        "order {order:?}"
                    );
                    assert_eq!(state.operation().unwrap().wait_status, WaitStatus::TimedOut);
                    assert_eq!(reports.len(), 2);
                    assert_eq!(
                        state.merge_notice(adopted.clone()).unwrap(),
                        Change::default()
                    );
                    assert_eq!(
                        state.merge_notice(additional.clone()).unwrap(),
                        Change::default()
                    );
                    assert_eq!(state.take_adopted_report(), Some(adopted.report.clone()));
                    assert_eq!(state.take_adopted_report(), None);
                    assert_eq!(state.report(31.into()), Some(&additional.report));
                }
            }
        }
    }
}

#[test]
fn conflicts_and_capacity_are_atomic_and_a_bare_report_cannot_choose_adoption() {
    let (accepted, _, adopted, additional) = facts();
    let mut state = State::new(2.into(), 10.into(), 1).unwrap();
    state.merge_operation(accepted.clone()).unwrap();
    state.merge_report(additional.report.clone()).unwrap();
    assert_eq!(state.take_adopted_report(), None);
    assert_eq!(
        state.merge_notice(adopted.clone()),
        Err(MergeError::Capacity)
    );
    assert_eq!(
        state.operation(),
        Some(&accepted),
        "capacity cannot partially advance the snapshot"
    );
    let mut conflict = additional.clone();
    conflict.report.report.result = ExecutionResult::Failed {
        error: "changed".to_owned(),
    };
    assert_eq!(state.merge_notice(conflict), Err(MergeError::Conflict));
    assert_eq!(state.operation(), Some(&accepted));
    state.merge_notice(additional.clone()).unwrap();
    assert_eq!(
        state.missing_report(),
        Some(30.into()),
        "additional evidence does not replace the adopted report"
    );
    let mut conflict = additional.clone();
    conflict.operation.adopted_report = Some(31.into());
    assert_eq!(state.merge_notice(conflict), Err(MergeError::Conflict));
    let mut conflict = additional.clone();
    conflict.operation.task = "changed task".to_owned();
    assert_eq!(state.merge_notice(conflict), Err(MergeError::Conflict));
    let mut invalid = additional.clone();
    invalid.report.worker_instance_id = 4.into();
    assert_eq!(state.merge_notice(invalid), Err(MergeError::Invalid));
    let mut invalid = additional.clone();
    invalid.report.report.result = ExecutionResult::Rejected {
        error: "before start".to_owned(),
    };
    assert_eq!(state.merge_notice(invalid), Err(MergeError::Invalid));
    assert_eq!(state.operation(), Some(&additional.operation));
    assert_eq!(state.report(31.into()), Some(&additional.report));
}

struct Wire {
    incoming: tokio::sync::Mutex<mpsc::UnboundedReceiver<Incoming>>,
    sent: Arc<Mutex<Vec<Vec<u8>>>>,
}
impl Wire {
    async fn receive(&self) -> Incoming {
        self.incoming
            .lock()
            .await
            .recv()
            .await
            .unwrap_or_else(|| Err(io::Error::other("closed")))
    }
}
impl Transport for &Wire {
    type Error = io::Error;
    async fn send(&self, bytes: &[u8]) -> Result<(), io::Error> {
        self.sent.lock().unwrap().push(bytes.to_vec());
        Ok(())
    }
    async fn recv(&self) -> Incoming {
        self.receive().await
    }
}
impl observe::Notifications for Wire {
    type Error = io::Error;
    async fn recv(&self) -> Incoming {
        self.receive().await
    }
}
fn channel() -> (Wire, mpsc::UnboundedSender<Incoming>) {
    let (tx, rx) = mpsc::unbounded_channel();
    (
        Wire {
            incoming: tokio::sync::Mutex::new(rx),
            sent: Arc::default(),
        },
        tx,
    )
}
fn send(tx: &mpsc::UnboundedSender<Incoming>, tag: u8) {
    tx.send(Ok(Some(vec![tag]))).unwrap();
}

struct Encoding {
    replies: Vec<Answer>,
    notices: Vec<Fact>,
}
impl query::Codec for Encoding {
    type Task = String;
    type Output = String;
    type Failure = String;
    type Error = io::Error;
    fn encode_query(&self, request: Request) -> Result<Vec<u8>, io::Error> {
        let (kind, id) = match request.selection {
            Selection::Operation(id) => (0, id.as_u128()),
            Selection::Report(id) => (1, id.as_u128()),
            Selection::Submission(_) => panic!("observation cannot submit or recover by a new key"),
        };
        Ok(vec![
            u8::try_from(request.id.as_u128()).unwrap(),
            kind,
            u8::try_from(id).unwrap(),
        ])
    }
    fn decode_query(&self, bytes: &[u8]) -> query::DecodeResult<Self> {
        bytes
            .first()
            .and_then(|index| self.replies.get(usize::from(*index)))
            .cloned()
            .ok_or_else(|| io::Error::other("invalid reply"))
    }
}
impl observe::Codec for Encoding {
    fn decode_notice(&self, bytes: &[u8]) -> observe::DecodeResult<Self> {
        bytes
            .first()
            .and_then(|index| self.notices.get(usize::from(*index)))
            .cloned()
            .ok_or_else(|| io::Error::other("invalid notice"))
    }
}
fn reply(id: u128, observation: Observation<String, String, String>) -> Answer {
    let selection = match &observation {
        Observation::Report(report) => Selection::Report(report.report.report_id),
        _ => Selection::Operation(10.into()),
    };
    Reply {
        request: Request {
            id: id.into(),
            authority_instance_id: 2.into(),
            selection,
        },
        observation,
    }
}
fn ids(start: u128) -> impl FnMut() -> query::Id {
    let mut id = start;
    move || {
        let next = id;
        id += 1;
        next.into()
    }
}

#[tokio::test(start_paused = true)]
async fn completion_before_subscription_is_queried_and_not_delivered_twice() {
    let (_, _, adopted, _) = facts();
    let (wire, tx) = channel();
    let (notices, _notice_tx) = channel();
    let codec = Encoding {
        replies: vec![
            reply(100, Observation::Operation(adopted.operation.clone())),
            reply(101, Observation::Report(adopted.report.clone())),
        ],
        notices: vec![],
    };
    send(&tx, 0);
    send(&tx, 1);
    let mut client = Client::new(&wire, codec, Some(2.into()));
    let mut state = State::new(2.into(), 10.into(), 2).unwrap();
    assert_eq!(
        client
            .observe(&mut state, &notices, Budget::default(), ids(100))
            .await
            .unwrap(),
        Some(adopted.report)
    );
    assert_eq!(
        *wire.sent.lock().unwrap(),
        vec![vec![100, 0, 10], vec![101, 1, 30]]
    );
    assert_eq!(
        client
            .observe(&mut state, &notices, Budget::default(), ids(102))
            .await
            .unwrap(),
        None
    );
    assert_eq!(wire.sent.lock().unwrap().len(), 2);
}

#[tokio::test(start_paused = true)]
async fn missing_notification_is_reconciled_even_while_the_connection_stays_open() {
    let (_, pending, adopted, _) = facts();
    let (wire, tx) = channel();
    let (notices, _notice_tx) = channel();
    let codec = Encoding {
        replies: vec![
            reply(100, Observation::Operation(pending)),
            reply(101, Observation::Operation(adopted.operation.clone())),
            reply(102, Observation::Report(adopted.report.clone())),
        ],
        notices: vec![],
    };
    for tag in [0, 1, 2] {
        send(&tx, tag);
    }
    let mut client = Client::new(&wire, codec, Some(2.into()));
    let mut state = State::new(2.into(), 10.into(), 2).unwrap();
    let start = Instant::now();
    assert_eq!(
        client
            .observe(&mut state, &notices, Budget::default(), ids(100))
            .await
            .unwrap(),
        Some(adopted.report)
    );
    assert_eq!(start.elapsed(), Duration::from_secs(1));
    assert_eq!(wire.sent.lock().unwrap().len(), 3);
}

#[tokio::test(start_paused = true)]
async fn duplicate_additional_notices_and_stale_snapshots_preserve_the_adopted_result() {
    let (accepted, pending, adopted, additional) = facts();
    let (wire, tx) = channel();
    let (notices, notice_tx) = channel();
    let mut foreign = adopted.clone();
    foreign.operation.binding.operation_id = 99.into();
    let codec = Encoding {
        replies: vec![
            reply(100, Observation::Operation(pending)),
            reply(101, Observation::Operation(accepted)),
            reply(102, Observation::Report(adopted.report.clone())),
        ],
        notices: vec![additional.clone(), foreign],
    };
    for tag in [0, 1, 2] {
        send(&tx, tag);
    }
    let producer = async {
        sleep(Duration::from_millis(100)).await;
        for tag in [255, 1, 0, 0] {
            send(&notice_tx, tag);
        }
    };
    let mut client = Client::new(&wire, codec, Some(2.into()));
    let mut state = State::new(2.into(), 10.into(), 2).unwrap();
    let (result, ()) = tokio::join!(
        client.observe(&mut state, &notices, Budget::default(), ids(100)),
        producer
    );
    assert_eq!(result.unwrap(), Some(adopted.report));
    assert_eq!(state.operation().unwrap().phase, OperationPhase::Reported);
    assert_eq!(state.report(31.into()), Some(&additional.report));
    assert_eq!(
        *wire.sent.lock().unwrap(),
        vec![vec![100, 0, 10], vec![101, 0, 10], vec![102, 1, 30]]
    );
}

#[tokio::test(start_paused = true)]
async fn a_full_notice_can_finish_while_a_query_is_waiting() {
    let (_, _, adopted, _) = facts();
    let (wire, _tx) = channel();
    let (notices, notice_tx) = channel();
    let codec = Encoding {
        replies: vec![],
        notices: vec![adopted.clone()],
    };
    let mut client = Client::new(&wire, codec, Some(2.into()));
    let mut state = State::new(2.into(), 10.into(), 2).unwrap();
    let producer = async {
        sleep(Duration::from_millis(100)).await;
        send(&notice_tx, 0);
    };
    let start = Instant::now();
    let (result, ()) = tokio::join!(
        client.observe(&mut state, &notices, Budget::default(), ids(100)),
        producer
    );
    assert_eq!(result.unwrap(), Some(adopted.report));
    assert_eq!(start.elapsed(), Duration::from_millis(100));
    assert_eq!(wire.sent.lock().unwrap().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn reconnect_uses_the_same_state_and_fetches_only_the_missing_report() {
    let (_, _, adopted, _) = facts();
    let (first, tx) = channel();
    let (notices, _notice_tx) = channel();
    let mut client = Client::new(
        &first,
        Encoding {
            replies: vec![reply(
                100,
                Observation::Operation(adopted.operation.clone()),
            )],
            notices: vec![],
        },
        Some(2.into()),
    );
    send(&tx, 0);
    tx.send(Err(io::Error::other("disconnected"))).unwrap();
    let mut state = State::new(2.into(), 10.into(), 2).unwrap();
    assert!(matches!(
        client
            .observe(&mut state, &notices, Budget::default(), ids(100))
            .await,
        Err(observe::Error::Query(query::ClientError::Transport { .. }))
    ));
    assert_eq!(state.operation(), Some(&adopted.operation));
    let (second, tx) = channel();
    let (notices, _notice_tx) = channel();
    let mut client = Client::new(
        &second,
        Encoding {
            replies: vec![reply(200, Observation::Report(adopted.report.clone()))],
            notices: vec![],
        },
        Some(2.into()),
    );
    send(&tx, 0);
    assert_eq!(
        client
            .observe(&mut state, &notices, Budget::default(), ids(200))
            .await
            .unwrap(),
        Some(adopted.report)
    );
    assert_eq!(*second.sent.lock().unwrap(), vec![vec![200, 1, 30]]);
}

#[tokio::test(start_paused = true)]
async fn total_deadline_bounds_queries_despite_noise_and_cancellation_stops_io() {
    let (wire, _tx) = channel();
    let (notices, notice_tx) = channel();
    let mut client = Client::new(
        &wire,
        Encoding {
            replies: vec![],
            notices: vec![],
        },
        Some(2.into()),
    );
    let mut state = State::new(2.into(), 10.into(), 2).unwrap();
    let budget = Budget {
        timeout: Duration::from_secs(1),
        query_timeout: Duration::from_millis(100),
        poll_interval: Duration::from_millis(200),
    };
    let noise = async {
        for _ in 0..9 {
            sleep(Duration::from_millis(100)).await;
            send(&notice_tx, 255);
        }
    };
    let start = Instant::now();
    let (result, ()) = tokio::join!(
        client.observe(&mut state, &notices, budget, ids(100)),
        noise
    );
    assert!(matches!(result, Err(observe::Error::TimedOut)));
    assert_eq!(start.elapsed(), Duration::from_secs(1));
    assert_eq!(
        *wire.sent.lock().unwrap(),
        vec![
            vec![100, 0, 10],
            vec![101, 0, 10],
            vec![102, 0, 10],
            vec![103, 0, 10]
        ]
    );
    assert!(state.operation().is_none());
    let cancelled = tokio::time::timeout(
        Duration::from_millis(50),
        client.observe(&mut state, &notices, budget, ids(110)),
    )
    .await;
    assert!(cancelled.is_err());
    let count = wire.sent.lock().unwrap().len();
    sleep(Duration::from_secs(2)).await;
    assert_eq!(
        wire.sent.lock().unwrap().len(),
        count,
        "dropping the wait must leave no background queries"
    );
}

#[tokio::test(start_paused = true)]
async fn not_found_and_notification_failure_do_not_erase_known_facts() {
    let (accepted, _, _, _) = facts();
    let (wire, tx) = channel();
    let (notices, notice_tx) = channel();
    let mut client = Client::new(
        &wire,
        Encoding {
            replies: vec![reply(100, Observation::NotFound)],
            notices: vec![],
        },
        Some(2.into()),
    );
    let mut state = State::new(2.into(), 10.into(), 2).unwrap();
    state.merge_operation(accepted.clone()).unwrap();
    send(&tx, 0);
    assert!(matches!(
        client
            .observe(&mut state, &notices, Budget::default(), ids(100))
            .await,
        Err(observe::Error::Query(query::ClientError::NotFound { .. }))
    ));
    assert_eq!(state.operation(), Some(&accepted));
    notice_tx
        .send(Err(io::Error::other("closed notices")))
        .unwrap();
    assert!(matches!(
        client
            .observe(&mut state, &notices, Budget::default(), ids(101))
            .await,
        Err(observe::Error::Notifications(_))
    ));
    assert_eq!(state.operation(), Some(&accepted));
    let sent = wire.sent.lock().unwrap().len();
    let invalid = Budget {
        timeout: Duration::ZERO,
        ..Budget::default()
    };
    assert!(matches!(
        client
            .observe(&mut state, &notices, invalid, ids(102))
            .await,
        Err(observe::Error::InvalidBudget)
    ));
    let mut client = Client::new(
        &wire,
        Encoding {
            replies: vec![],
            notices: vec![],
        },
        Some(9.into()),
    );
    assert!(matches!(
        client
            .observe(&mut state, &notices, Budget::default(), ids(102))
            .await,
        Err(observe::Error::WrongAuthority)
    ));
    assert_eq!(wire.sent.lock().unwrap().len(), sent);
}

struct Flood;
impl Transport for Flood {
    type Error = io::Error;
    async fn send(&self, _: &[u8]) -> Result<(), io::Error> {
        Ok(())
    }
    async fn recv(&self) -> Incoming {
        Ok(None)
    }
}
impl observe::Notifications for Flood {
    type Error = io::Error;
    async fn recv(&self) -> Incoming {
        Ok(None)
    }
}

#[tokio::test]
async fn continuously_ready_invalid_traffic_cannot_starve_the_total_deadline() {
    // A current-thread runtime must still poll its timers when neither source ever blocks.
    for flood_queries in [false, true] {
        let (wire, _tx) = channel();
        let (notices, _notice_tx) = channel();
        let mut normal = Client::new(
            &wire,
            Encoding {
                replies: vec![],
                notices: vec![],
            },
            Some(2.into()),
        );
        let mut flooded = Client::new(
            Flood,
            Encoding {
                replies: vec![],
                notices: vec![],
            },
            Some(2.into()),
        );
        let mut state = State::new(2.into(), 10.into(), 2).unwrap();
        let budget = Budget {
            timeout: Duration::from_millis(50),
            query_timeout: Duration::from_millis(10),
            poll_interval: Duration::from_millis(10),
        };
        let started = Instant::now();
        let result = if flood_queries {
            flooded
                .observe(&mut state, &notices, budget, ids(100))
                .await
        } else {
            normal.observe(&mut state, &Flood, budget, ids(100)).await
        };
        assert!(matches!(result, Err(observe::Error::TimedOut)));
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "ready traffic must leave the runtime responsive"
        );
    }
}
