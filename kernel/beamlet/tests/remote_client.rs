use beamlet::{
    ReportObservation,
    id::SubmissionKey,
    model::{ExecutionResult, ImpactStatus, SubmitReport},
    remote::{Binding, Client, ClientError, Codec, Outcome, Reply, Transport},
};
use std::{
    collections::HashMap,
    io,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{
    sync::{Notify, mpsc},
    time::Instant,
};

type Response = Reply<String, String>;
type Incoming = Result<Option<Vec<u8>>, io::Error>;

#[derive(Clone, Copy)]
enum SendMode {
    Succeed,
    Fail,
    Stall,
}

struct Sent {
    at: Instant,
    bytes: Vec<u8>,
}

struct Wire {
    incoming: tokio::sync::Mutex<mpsc::UnboundedReceiver<Incoming>>,
    sent: Arc<Mutex<Vec<Sent>>>,
    ready: Arc<Notify>,
    mode: SendMode,
}

impl Transport for Wire {
    type Error = io::Error;
    async fn send(&self, bytes: &[u8]) -> Result<(), io::Error> {
        self.sent.lock().unwrap().push(Sent {
            at: Instant::now(),
            bytes: bytes.to_vec(),
        });
        self.ready.notify_one();
        match self.mode {
            SendMode::Succeed => Ok(()),
            SendMode::Fail => Err(io::Error::other("send unconfirmed")),
            SendMode::Stall => std::future::pending().await,
        }
    }
    async fn recv(&self) -> Result<Option<Vec<u8>>, io::Error> {
        self.incoming
            .lock()
            .await
            .recv()
            .await
            .unwrap_or_else(|| Err(io::Error::other("closed")))
    }
}

struct Encoding {
    replies: HashMap<u8, Response>,
    encodings: Arc<AtomicUsize>,
}

impl Codec for Encoding {
    type Task = str;
    type Output = String;
    type Failure = String;
    type Error = io::Error;
    fn encode(&self, key: SubmissionKey, task: &str) -> Result<Vec<u8>, io::Error> {
        self.encodings.fetch_add(1, Ordering::SeqCst);
        if task.is_empty() {
            return Err(io::Error::other("invalid task"));
        }
        Ok(format!("{key}:{task}").into_bytes())
    }
    fn decode(&self, bytes: &[u8]) -> Result<Response, io::Error> {
        bytes
            .first()
            .and_then(|tag| self.replies.get(tag))
            .cloned()
            .ok_or_else(|| io::Error::other("malformed"))
    }
}

struct Harness {
    tx: mpsc::UnboundedSender<Incoming>,
    sent: Arc<Mutex<Vec<Sent>>>,
    ready: Arc<Notify>,
    encodings: Arc<AtomicUsize>,
}

fn client(
    replies: impl IntoIterator<Item = (u8, Response)>,
    mode: SendMode,
) -> (Client<Wire, Encoding>, Harness) {
    let (tx, rx) = mpsc::unbounded_channel();
    let sent = Arc::new(Mutex::new(Vec::new()));
    let ready = Arc::new(Notify::new());
    let encodings = Arc::new(AtomicUsize::new(0));
    let wire = Wire {
        incoming: tokio::sync::Mutex::new(rx),
        sent: sent.clone(),
        ready: ready.clone(),
        mode,
    };
    let codec = Encoding {
        replies: replies.into_iter().collect(),
        encodings: encodings.clone(),
    };
    (
        Client::new(wire, codec, Some(2.into())),
        Harness {
            tx,
            sent,
            ready,
            encodings,
        },
    )
}

fn binding() -> Binding {
    Binding {
        authority_instance_id: 2.into(),
        operation_id: 10.into(),
        attempt_id: 20.into(),
    }
}

fn accepted(outcome: Option<Outcome<String, String>>) -> Response {
    Reply::Accepted {
        key: 7.into(),
        binding: binding(),
        outcome,
    }
}

fn evidence() -> ReportObservation<String, String> {
    ReportObservation::new(
        2.into(),
        3.into(),
        SubmitReport {
            report_id: 30.into(),
            operation_id: 10.into(),
            attempt_id: 20.into(),
            impact: ImpactStatus::Unknown,
            result: ExecutionResult::Failed {
                error: "tool failure".to_owned(),
            },
        },
    )
}

#[tokio::test(start_paused = true)]
async fn sends_identical_bytes_with_fixed_attempt_and_total_time_budgets() {
    for mode in [SendMode::Succeed, SendMode::Fail, SendMode::Stall] {
        let (mut client, harness) = client([], mode);
        let started = Instant::now();
        let error = client.submit(7.into(), "task").await.unwrap_err();
        let ClientError::TimedOut { progress } = error else {
            panic!("expected unknown outcome at the deadline")
        };
        assert_eq!(progress.submission_key, 7.into());
        assert_eq!(progress.binding, None, "transport success is not admission");
        assert_eq!(progress.sends, 3);
        assert_eq!(started.elapsed(), Duration::from_secs(10));
        assert_eq!(harness.encodings.load(Ordering::SeqCst), 1);
        let sent = harness.sent.lock().unwrap();
        assert_eq!(sent.len(), 3);
        assert!(sent.iter().all(|message| message.bytes == sent[0].bytes));
        let interval = match mode {
            SendMode::Stall => 1500,
            _ => 1000,
        };
        assert_eq!(
            sent.iter()
                .map(|message| message.at.duration_since(started).as_millis())
                .collect::<Vec<_>>(),
            vec![0, interval, 2 * interval]
        );
    }
}

#[tokio::test(start_paused = true)]
async fn ignores_unrelated_invalid_and_conflicting_replies_and_preserves_business_failure() {
    let mut wrong_report = evidence();
    wrong_report.report.attempt_id = 99.into();
    let mut invalid_report = evidence();
    invalid_report.report.result = ExecutionResult::Rejected {
        error: "not started".to_owned(),
    };
    let replies = [
        (
            1,
            Reply::Accepted {
                key: 8.into(),
                binding: binding(),
                outcome: Some(Outcome::Pending),
            },
        ),
        (
            2,
            Reply::Rejected {
                key: 7.into(),
                authority_instance_id: 9.into(),
                reason: "wrong authority".to_owned(),
            },
        ),
        (3, accepted(Some(Outcome::Report(wrong_report)))),
        (4, accepted(Some(Outcome::Report(invalid_report)))),
        (5, accepted(None)),
        (
            6,
            Reply::Accepted {
                key: 7.into(),
                binding: Binding {
                    operation_id: 99.into(),
                    ..binding()
                },
                outcome: Some(Outcome::Pending),
            },
        ),
        (
            7,
            Reply::Rejected {
                key: 7.into(),
                authority_instance_id: 2.into(),
                reason: "late rejection cannot erase admission".to_owned(),
            },
        ),
        (8, accepted(Some(Outcome::Report(evidence())))),
    ];
    let (mut client, harness) = client(replies, SendMode::Succeed);
    let producer = async {
        harness.ready.notified().await;
        harness.tx.send(Ok(None)).unwrap();
        for tag in 0..=8 {
            harness.tx.send(Ok(Some(vec![tag]))).unwrap();
        }
    };
    let task = "task";
    let (result, ()) = tokio::join!(client.submit(7.into(), task), producer);
    let submission = result.unwrap();
    assert_eq!(submission.binding, binding());
    assert_eq!(submission.outcome, Outcome::Report(evidence()));
    assert_eq!(harness.sent.lock().unwrap().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn report_can_confirm_admission_before_a_receipt_and_channel_can_be_reused() {
    let replies = [
        (1, accepted(Some(Outcome::Report(evidence())))),
        (2, accepted(None)),
        (
            3,
            Reply::Accepted {
                key: 8.into(),
                binding: Binding {
                    operation_id: 11.into(),
                    attempt_id: 21.into(),
                    ..binding()
                },
                outcome: Some(Outcome::Pending),
            },
        ),
    ];
    let (mut client, harness) = client(replies, SendMode::Succeed);
    let task = "task";
    let producer = async {
        harness.ready.notified().await;
        harness.tx.send(Ok(Some(vec![1]))).unwrap();
    };
    let (result, ()) = tokio::join!(client.submit(7.into(), task), producer);
    assert_eq!(result.unwrap().outcome, Outcome::Report(evidence()));
    // Reusing the same key must still send once, even with a stale receipt already queued.
    harness.tx.send(Ok(Some(vec![2]))).unwrap();
    tokio::time::advance(Duration::from_nanos(1)).await;
    let producer = async {
        harness.ready.notified().await;
        harness.tx.send(Ok(Some(vec![1]))).unwrap();
    };
    let (result, ()) = tokio::join!(client.submit(7.into(), task), producer);
    assert_eq!(result.unwrap().binding, binding());
    // A previous call's delayed receipt must not complete this new submission.
    harness.tx.send(Ok(Some(vec![2]))).unwrap();
    let producer = async {
        harness.ready.notified().await;
        harness.tx.send(Ok(Some(vec![3]))).unwrap();
    };
    let (result, ()) = tokio::join!(client.submit(8.into(), task), producer);
    let result = result.unwrap();
    assert_eq!(result.submission_key, 8.into());
    assert_eq!(result.binding.operation_id, 11.into());
    assert_eq!(result.outcome, Outcome::Pending);
    assert_eq!(harness.sent.lock().unwrap().len(), 3);
}

#[tokio::test(start_paused = true)]
async fn duplicate_receipts_stop_sends_without_extending_the_result_deadline() {
    let (mut client, harness) = client([(1, accepted(None))], SendMode::Succeed);
    let started = Instant::now();
    let producer = async {
        harness.ready.notified().await;
        harness.tx.send(Ok(Some(vec![1]))).unwrap();
        tokio::time::sleep(Duration::from_secs(9)).await;
        harness.tx.send(Ok(Some(vec![1]))).unwrap();
    };
    let task = "task";
    let (result, ()) = tokio::join!(client.submit(7.into(), task), producer);
    let ClientError::TimedOut { progress } = result.unwrap_err() else {
        panic!("admission alone cannot complete the call")
    };
    assert_eq!(progress.binding, Some(binding()));
    assert_eq!(progress.sends, 1);
    assert_eq!(started.elapsed(), Duration::from_secs(10));
}

#[tokio::test(start_paused = true)]
async fn local_encoding_rejection_and_receive_failure_have_distinct_results() {
    let (mut client, harness) = client([(1, accepted(None))], SendMode::Succeed);
    assert!(matches!(
        client.submit(7.into(), "").await,
        Err(ClientError::Encode(_))
    ));
    assert!(harness.sent.lock().unwrap().is_empty());
    let producer = async {
        harness.ready.notified().await;
        harness.tx.send(Ok(Some(vec![1]))).unwrap();
        harness
            .tx
            .send(Err(io::Error::other("connection lost")))
            .unwrap();
    };
    let task = "task";
    let (result, ()) = tokio::join!(client.submit(7.into(), task), producer);
    let ClientError::Transport { progress, source } = result.unwrap_err() else {
        panic!("expected connection error")
    };
    assert_eq!(progress.binding, Some(binding()));
    assert_eq!(source.to_string(), "connection lost");
}

#[tokio::test(start_paused = true)]
async fn rejection_is_not_a_report_and_cancelling_a_wait_leaves_no_background_retries() {
    let (mut rejected, harness) = client(
        [(
            1,
            Reply::Rejected {
                key: 7.into(),
                authority_instance_id: 2.into(),
                reason: "full".to_owned(),
            },
        )],
        SendMode::Succeed,
    );
    harness.tx.send(Ok(Some(vec![1]))).unwrap();
    assert!(
        matches!(rejected.submit(7.into(), "task").await, Err(ClientError::Rejected { submission_key, reason, .. }) if submission_key == 7.into() && reason == "full")
    );
    let (mut waiting, harness) = client([], SendMode::Succeed);
    assert!(
        tokio::time::timeout(Duration::from_millis(100), waiting.submit(7.into(), "task"))
            .await
            .is_err()
    );
    tokio::time::advance(Duration::from_secs(20)).await;
    assert_eq!(harness.sent.lock().unwrap().len(), 1);
}
