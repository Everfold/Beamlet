use beamlet::{
    Authority, ExecutionBinding, OperationObservation, ReportObservation,
    memory::{Admission, Limits},
    model::{Delivery, ExecutionResult, ImpactStatus, Message, SubmitReport},
    query::{ClientError, Codec, Observation, Reply, Request, Selection, WrongAuthority},
    remote::{Client, Transport},
    state::{OperationPhase, WaitStatus},
};
use std::{collections::VecDeque, io, sync::Mutex, time::Duration};
use tokio::time::Instant;

type Answer = Reply<String, String, String>;
type Incoming = Result<Option<Vec<u8>>, io::Error>;

fn request(selection: Selection) -> Request {
    Request {
        id: 100.into(),
        authority_instance_id: 2.into(),
        selection,
    }
}

fn view() -> OperationObservation<String> {
    OperationObservation {
        binding: ExecutionBinding {
            authority_instance_id: 2.into(),
            worker_instance_id: 3.into(),
            operation_id: 10.into(),
            attempt_id: 20.into(),
        },
        task: "task".to_owned(),
        phase: OperationPhase::Reported,
        wait_status: WaitStatus::TimedOut,
        adopted_report: Some(30.into()),
    }
}

fn report(id: u128) -> ReportObservation<String, String> {
    ReportObservation::new(
        2.into(),
        3.into(),
        SubmitReport {
            report_id: id.into(),
            operation_id: 10.into(),
            attempt_id: 20.into(),
            impact: ImpactStatus::Unknown,
            result: ExecutionResult::Failed {
                error: "tool failure".to_owned(),
            },
        },
    )
}

#[test]
fn authority_queries_preserve_pending_timeout_and_additional_evidence_without_admission() {
    let mut authority =
        Authority::<String, String, String>::new(2.into(), vec![3.into()], Limits::default())
            .unwrap();
    for selection in [
        Selection::Submission(7.into()),
        Selection::Operation(10.into()),
        Selection::Report(30.into()),
    ] {
        assert_eq!(
            authority
                .answer_query(request(selection))
                .unwrap()
                .observation,
            Observation::NotFound
        );
    }
    let accepted = authority
        .submit_keyed(7.into(), "task".to_owned(), || (10.into(), 20.into()))
        .unwrap();
    assert_eq!(
        accepted.admission,
        Admission::New,
        "queries cannot reserve a key or identity"
    );
    let selection = Selection::Submission(7.into());
    let initial = authority.answer_query(request(selection)).unwrap();
    let Observation::Operation(initial_view) = &initial.observation else {
        panic!("expected operation")
    };
    assert_eq!(initial_view.phase, OperationPhase::Accepted);
    assert_eq!(initial_view.wait_status, WaitStatus::Waiting);
    authority.mark_dispatched(10.into()).unwrap();
    authority.mark_wait_timeout(10.into()).unwrap();
    let pending = authority.answer_query(request(selection)).unwrap();
    let Observation::Operation(pending_view) = &pending.observation else {
        panic!("expected operation")
    };
    assert_eq!(pending_view.phase, OperationPhase::Dispatched);
    assert_eq!(pending_view.wait_status, WaitStatus::TimedOut);
    assert_eq!(pending_view.adopted_report, None);
    for id in [30, 31] {
        authority
            .receive_report(Delivery::new(
                3.into(),
                2.into(),
                Message::SubmitReport(report(id).report),
            ))
            .unwrap();
    }
    for selection in [
        Selection::Submission(7.into()),
        Selection::Operation(10.into()),
    ] {
        let observed = authority.answer_query(request(selection)).unwrap();
        assert_eq!(observed.observation, Observation::Operation(view()));
        assert!(observed.matches(request(selection)));
    }
    for id in [30, 31] {
        assert_eq!(
            authority
                .answer_query(request(Selection::Report(id.into())))
                .unwrap()
                .observation,
            Observation::Report(report(id))
        );
    }
    assert_eq!(
        authority.answer_query(Request {
            authority_instance_id: 9.into(),
            ..request(selection)
        }),
        Err(WrongAuthority)
    );
    assert_eq!(
        authority
            .answer_query(request(selection))
            .unwrap()
            .observation,
        Observation::Operation(view())
    );
    assert_eq!(
        initial_view.phase,
        OperationPhase::Accepted,
        "earlier snapshots stay detached"
    );
    assert_eq!(
        authority
            .submit_keyed(7.into(), "task".to_owned(), || panic!(
                "query did not remove the key"
            ))
            .unwrap()
            .admission,
        Admission::Duplicate
    );
}

struct Encoding {
    replies: Vec<Answer>,
}
impl Codec for Encoding {
    type Task = String;
    type Output = String;
    type Failure = String;
    type Error = io::Error;
    fn encode_query(&self, request: Request) -> Result<Vec<u8>, io::Error> {
        if request.id.as_u128() == 0 {
            return Err(io::Error::other("encoding rejected"));
        }
        Ok(request.id.as_u128().to_be_bytes().to_vec())
    }
    fn decode_query(&self, bytes: &[u8]) -> Result<Answer, io::Error> {
        bytes
            .first()
            .and_then(|index| self.replies.get(usize::from(*index)))
            .cloned()
            .ok_or_else(|| io::Error::other("malformed"))
    }
}

#[derive(Clone, Copy)]
enum SendMode {
    Success,
    Failure,
    Stall,
}
struct Wire {
    mode: SendMode,
    sent: Mutex<Vec<Vec<u8>>>,
    incoming: Mutex<VecDeque<Incoming>>,
}
impl Transport for &Wire {
    type Error = io::Error;
    async fn send(&self, bytes: &[u8]) -> Result<(), io::Error> {
        self.sent.lock().unwrap().push(bytes.to_vec());
        match self.mode {
            SendMode::Success => Ok(()),
            SendMode::Failure => Err(io::Error::other("send failed")),
            SendMode::Stall => std::future::pending().await,
        }
    }
    async fn recv(&self) -> Result<Option<Vec<u8>>, io::Error> {
        let next = self.incoming.lock().unwrap().pop_front();
        match next {
            Some(value) => value,
            None => std::future::pending().await,
        }
    }
}
fn wire(mode: SendMode, tags: impl IntoIterator<Item = u8>) -> Wire {
    Wire {
        mode,
        sent: Mutex::new(Vec::new()),
        incoming: Mutex::new(tags.into_iter().map(|tag| Ok(Some(vec![tag]))).collect()),
    }
}

#[tokio::test(start_paused = true)]
async fn stale_foreign_malformed_and_inconsistent_replies_do_not_satisfy_query() {
    let expected = request(Selection::Operation(10.into()));
    let valid = Reply {
        request: expected,
        observation: Observation::Operation(view()),
    };
    let mut wrong_id = valid.clone();
    wrong_id.request.id = 99.into();
    let mut wrong_authority = valid.clone();
    wrong_authority.request.authority_instance_id = 9.into();
    let mut wrong_selection = valid.clone();
    wrong_selection.request.selection = Selection::Submission(7.into());
    let mut wrong_binding = view();
    wrong_binding.binding.operation_id = 11.into();
    let mut wrong_phase = view();
    wrong_phase.phase = OperationPhase::Accepted;
    let replies = vec![
        wrong_id,
        wrong_authority,
        wrong_selection,
        Reply {
            request: expected,
            observation: Observation::Operation(wrong_binding),
        },
        Reply {
            request: expected,
            observation: Observation::Operation(wrong_phase),
        },
        Reply {
            request: expected,
            observation: Observation::Report(report(30)),
        },
        valid,
    ];
    let wire = wire(SendMode::Success, [255, 0, 1, 2, 3, 4, 5, 6]);
    let mut client = Client::new(&wire, Encoding { replies }, Some(2.into()));
    assert_eq!(
        client.query_operation(100.into(), 10.into()).await.unwrap(),
        view()
    );
    assert!(
        wire.incoming.lock().unwrap().is_empty(),
        "only the final reply is valid"
    );
    assert_eq!(wire.sent.lock().unwrap().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn repeated_selection_requires_a_new_reply_and_preserves_failed_report() {
    let selection = Selection::Submission(7.into());
    let old = Reply {
        request: request(selection),
        observation: Observation::Operation(view()),
    };
    let mut current = old.clone();
    current.request.id = 101.into();
    let evidence = Reply {
        request: Request {
            id: 102.into(),
            ..request(Selection::Report(30.into()))
        },
        observation: Observation::Report(report(30)),
    };
    let wire = wire(SendMode::Success, [0, 0, 1, 2]);
    let mut client = Client::new(
        &wire,
        Encoding {
            replies: vec![old, current, evidence],
        },
        Some(2.into()),
    );
    assert_eq!(
        client.query_submission(100.into(), 7.into()).await.unwrap(),
        view()
    );
    assert_eq!(
        client.query_submission(101.into(), 7.into()).await.unwrap(),
        view()
    );
    assert_eq!(
        client.query_report(102.into(), 30.into()).await.unwrap(),
        report(30)
    );
    assert_eq!(wire.sent.lock().unwrap().len(), 3);
}

#[tokio::test(start_paused = true)]
async fn only_matching_not_found_is_absence_and_transport_failure_stays_unknown() {
    let expected = request(Selection::Report(30.into()));
    let wire = wire(SendMode::Success, [0, 1]);
    let mut client = Client::new(
        &wire,
        Encoding {
            replies: vec![
                Reply {
                    request: Request {
                        id: 99.into(),
                        ..expected
                    },
                    observation: Observation::NotFound,
                },
                Reply {
                    request: expected,
                    observation: Observation::NotFound,
                },
            ],
        },
        Some(2.into()),
    );
    assert!(
        matches!(client.query_report(100.into(), 30.into()).await, Err(ClientError::NotFound { request }) if request == expected)
    );
    wire.incoming
        .lock()
        .unwrap()
        .push_back(Err(io::Error::other("connection lost")));
    assert!(matches!(
        client.query_report(101.into(), 30.into()).await,
        Err(ClientError::Transport { .. })
    ));
}

#[tokio::test(start_paused = true)]
async fn one_send_and_one_total_deadline_never_retry_or_return_not_found() {
    for mode in [SendMode::Success, SendMode::Failure, SendMode::Stall] {
        let wire = wire(mode, [255, 255]);
        let mut client = Client::new(
            &wire,
            Encoding {
                replies: Vec::new(),
            },
            Some(2.into()),
        );
        let started = Instant::now();
        let result = client.query_operation(100.into(), 10.into()).await;
        match mode {
            SendMode::Failure => assert!(matches!(result, Err(ClientError::Transport { .. }))),
            _ => {
                assert!(matches!(result, Err(ClientError::TimedOut { .. })));
                assert_eq!(started.elapsed(), Duration::from_secs(10));
            }
        }
        assert_eq!(wire.sent.lock().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn explicit_authority_and_valid_encoding_are_required_before_io() {
    let wire = wire(SendMode::Success, []);
    let mut client = Client::new(
        &wire,
        Encoding {
            replies: Vec::new(),
        },
        None,
    );
    assert!(matches!(
        client.query_operation(100.into(), 10.into()).await,
        Err(ClientError::AuthorityRequired)
    ));
    let mut client = Client::new(
        &wire,
        Encoding {
            replies: Vec::new(),
        },
        Some(2.into()),
    );
    assert!(matches!(
        client.query_operation(0.into(), 10.into()).await,
        Err(ClientError::Encode(_))
    ));
    assert!(wire.sent.lock().unwrap().is_empty());
}
