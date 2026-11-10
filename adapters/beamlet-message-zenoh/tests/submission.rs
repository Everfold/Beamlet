use beamlet::{
    Authority,
    memory::{Admission, Limits as LedgerLimits},
    model::{Delivery, ExecutionResult, ImpactStatus, Message, SubmitReport},
    remote::{Client, Outcome},
};
use beamlet_agent::testing::{
    hello::{Response, Task},
    protocol::{Limits as ProtocolLimits, Uuid},
    submission::{HelloSubmissionCodec, Submission, SubmissionReply},
};
use beamlet_message_zenoh::{Limits, SubmissionTransport, Transport};
use std::{net::TcpListener, time::Duration};
use tokio::time::timeout;

async fn listener() -> (Transport, String) {
    for _ in 0..5 {
        let socket = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("tcp/{}", socket.local_addr().unwrap());
        drop(socket);
        if let Ok(transport) =
            Transport::listen(&endpoint, "beamlet/test/submissions", Limits::default()).await
        {
            return (transport, endpoint);
        }
    }
    panic!("test listener must bind after bounded port-selection retries");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn library_client_reuses_explicit_session_and_does_not_own_shared_subscriptions() {
    let (server, endpoint) = listener().await;
    let mut connection = Transport::connect(&endpoint, "beamlet/test/replies", Limits::default())
        .await
        .unwrap();
    let notices = connection.subscribe("beamlet/test/notices").await.unwrap();
    let to_client = server.sender("beamlet/test/replies").await.unwrap();
    let to_notices = server.sender("beamlet/test/notices").await.unwrap();
    timeout(Duration::from_secs(10), async {
        to_client.wait_for_receiver().await.unwrap();
        to_notices.wait_for_receiver().await.unwrap();
        let service = async {
            let limits = ProtocolLimits::default();
            let mut authority = Authority::<Task, String, String>::new(
                2.into(),
                vec![3.into()],
                LedgerLimits::default(),
            )
            .unwrap();
            let mut allocations = 0_u128;
            for _ in 0..3 {
                let request =
                    Submission::from_json(&server.recv().await.unwrap(), &limits).unwrap();
                let accepted = authority
                    .submit_keyed(
                        request.submission_key.as_u128().into(),
                        request.task,
                        || {
                            allocations += 1;
                            ((10 + allocations).into(), (20 + allocations).into())
                        },
                    )
                    .unwrap();
                let Message::DispatchAttempt(attempt) = &accepted.dispatch.message else {
                    panic!("expected dispatch")
                };
                if accepted.admission == Admission::New {
                    authority
                        .receive_report(Delivery::new(
                            3.into(),
                            2.into(),
                            Message::SubmitReport(SubmitReport {
                                report_id: (30 + allocations).into(),
                                operation_id: attempt.operation.operation_id,
                                attempt_id: attempt.attempt_id,
                                impact: ImpactStatus::None,
                                result: ExecutionResult::Succeeded {
                                    output: "World".to_owned(),
                                },
                            }),
                        ))
                        .unwrap();
                }
                let view = authority.operation(attempt.operation.operation_id).unwrap();
                let Message::SubmitReport(report) = &authority
                    .report(view.adopted_report.unwrap())
                    .unwrap()
                    .message
                else {
                    panic!("expected retained report")
                };
                let receipt =
                    SubmissionReply::accepted(request.submission_key, &accepted.dispatch).unwrap();
                // The result itself confirms admission; the separate receipt arrives late.
                to_client
                    .send(
                        receipt
                            .with_response(Response::report(report, 3.into(), 2.into()))
                            .to_json(&limits)
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                to_client
                    .send(receipt.to_json(&limits).unwrap())
                    .await
                    .unwrap();
            }
            assert_eq!(
                allocations, 2,
                "the same key must reuse the original execution"
            );
        };
        let application = async {
            {
                let channel =
                    SubmissionTransport::bind(&mut connection, "beamlet/test/submissions")
                        .await
                        .unwrap();
                channel.wait_for_receiver().await.unwrap();
                let mut client =
                    Client::new(channel, HelloSubmissionCodec::default(), Some(2.into()));
                let mut bindings = Vec::new();
                for key in [7, 7, 8] {
                    let result = client.submit(key.into(), &Task::Hello).await.unwrap();
                    let Outcome::Report(observed) = result.outcome else {
                        panic!("expected report")
                    };
                    assert_eq!(
                        observed.report.result,
                        ExecutionResult::Succeeded {
                            output: "World".to_owned()
                        }
                    );
                    assert_eq!(observed.authority_instance_id, 2.into());
                    assert_eq!(observed.worker_instance_id, 3.into());
                    bindings.push(result.binding);
                }
                assert_eq!(bindings[0], bindings[1]);
                assert_ne!(bindings[0], bindings[2]);
                to_notices.send(b"while bound".to_vec()).await.unwrap();
                assert_eq!(notices.recv().await.unwrap(), b"while bound");
            }
            // Dropping Client releases its borrow, while the app's session stays usable.
            let publisher = connection.sender("beamlet/test/notices").await.unwrap();
            publisher.wait_for_receiver().await.unwrap();
            publisher.send(b"after drop".to_vec()).await.unwrap();
            assert_eq!(notices.recv().await.unwrap(), b"after drop");
        };
        tokio::join!(service, application);
    })
    .await
    .expect("library submission must complete without a Console or readiness sleeps");
}

#[test]
fn hello_codec_preserves_key_and_rejects_mismatched_report_binding() {
    use beamlet::remote::{Codec, Reply};
    let codec = HelloSubmissionCodec::default();
    let limits = ProtocolLimits::default();
    let request = codec.encode(7.into(), &Task::Hello).unwrap();
    assert_eq!(
        Submission::from_json(&request, &limits)
            .unwrap()
            .submission_key,
        Uuid::from_u128(7)
    );
    let mut authority =
        Authority::<Task, String, String>::new(2.into(), vec![3.into()], LedgerLimits::default())
            .unwrap();
    let accepted = authority
        .submit_keyed(7.into(), Task::Hello, || (10.into(), 20.into()))
        .unwrap();
    let reply = SubmissionReply::accepted(Uuid::from_u128(7), &accepted.dispatch).unwrap();
    assert!(matches!(
        codec.decode(&reply.to_json(&limits).unwrap()).unwrap(),
        Reply::Accepted { outcome: None, .. }
    ));
    let report = SubmitReport {
        report_id: 30.into(),
        operation_id: 10.into(),
        attempt_id: 20.into(),
        impact: ImpactStatus::None,
        result: ExecutionResult::Succeeded {
            output: "World".to_owned(),
        },
    };
    let bytes = reply
        .with_response(Response::report(&report, 3.into(), 2.into()))
        .to_json(&limits)
        .unwrap();
    let mut invalid: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    invalid["attempt_id"] = serde_json::json!(Uuid::from_u128(99));
    assert!(
        codec
            .decode(&serde_json::to_vec(&invalid).unwrap())
            .is_err()
    );
    assert!(codec.decode(b"{broken json").is_err());
}
