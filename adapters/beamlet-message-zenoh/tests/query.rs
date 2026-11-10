use beamlet::{
    Authority, LocalClient, LocalExecution,
    memory::{Admission, Limits as LedgerLimits},
    model::{Delivery, ExecutionResult, ImpactStatus, Message, SubmitReport},
    query::{ClientError, Codec, Observation, Request, Selection},
    remote::Client,
};
use beamlet_message_zenoh::{ClientTransport, Limits, Transport};
use beamlet_protocol_demo::{
    hello::Task, protocol::Limits as ProtocolLimits, query, submission::HelloClientCodec,
};
use std::{net::TcpListener, time::Duration};
use tokio::time::timeout;

fn authority() -> Authority<Task, String, String> {
    let mut authority = Authority::new(2.into(), vec![3.into()], LedgerLimits::default()).unwrap();
    authority
        .submit_keyed(7.into(), Task::Hello, || (10.into(), 20.into()))
        .unwrap();
    authority.mark_dispatched(10.into()).unwrap();
    authority.mark_wait_timeout(10.into()).unwrap();
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
                        error: "observed failure".to_owned(),
                    },
                }),
            ))
            .unwrap();
    }
    authority
}

async fn listener() -> (Transport, String) {
    for _ in 0..5 {
        let socket = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("tcp/{}", socket.local_addr().unwrap());
        drop(socket);
        if let Ok(transport) =
            Transport::listen(&endpoint, "beamlet/test/queries", Limits::default()).await
        {
            return (transport, endpoint);
        }
    }
    panic!("test listener must bind after bounded port-selection retries");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fresh_connections_recover_original_records_with_identical_local_facts() {
    let mut authority = authority();
    let (_, mut worker) = LocalExecution::<Task, String, String>::new(
        2.into(),
        vec![3.into()],
        LedgerLimits::default(),
    )
    .unwrap()
    .into_parts();
    let local = LocalClient::new(&mut authority, &mut worker, || {
        panic!("queries cannot allocate")
    });
    let expected = local.query_submission(7.into()).unwrap();
    assert_eq!(expected, local.query_operation(10.into()).unwrap());
    let additional = local.query_report(31.into()).unwrap();
    let (server, endpoint) = listener().await;
    timeout(Duration::from_secs(20), async {
        for index in 0..2_u128 {
            // Destroy the whole connection between rounds; retain only original lookup IDs.
            let inbox = format!("beamlet/test/query-replies/{index}");
            let mut connection = Transport::connect(&endpoint, &inbox, Limits::default())
                .await
                .unwrap();
            let to_client = server.sender(&inbox).await.unwrap();
            to_client.wait_for_receiver().await.unwrap();
            let service = async {
                for _ in 0..4 {
                    let bytes = server.recv().await.unwrap();
                    let reply =
                        query::respond(&authority, &bytes, &ProtocolLimits::default()).unwrap();
                    to_client.send(reply).await.unwrap();
                }
            };
            let application = async {
                let channel = ClientTransport::bind(&mut connection, "beamlet/test/queries")
                    .await
                    .unwrap();
                let mut client = Client::new(channel, HelloClientCodec::default(), Some(2.into()));
                assert_eq!(
                    client
                        .query_submission((100 + index * 4).into(), 7.into())
                        .await
                        .unwrap(),
                    expected
                );
                assert_eq!(
                    client
                        .query_operation((101 + index * 4).into(), 10.into())
                        .await
                        .unwrap(),
                    expected
                );
                assert_eq!(
                    client
                        .query_report((102 + index * 4).into(), 31.into())
                        .await
                        .unwrap(),
                    additional
                );
                assert!(matches!(
                    client
                        .query_submission((103 + index * 4).into(), 8.into())
                        .await,
                    Err(ClientError::NotFound { .. })
                ));
            };
            tokio::join!(service, application);
        }
    })
    .await
    .expect("reconnected query clients must complete without readiness sleeps");
    assert!(
        worker
            .next_report(|_| async { panic!("queries cannot queue work") })
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        authority
            .submit_keyed(7.into(), Task::Hello, || panic!("original key remains"))
            .unwrap()
            .admission,
        Admission::Duplicate
    );
    assert_eq!(
        authority
            .submit_keyed(8.into(), Task::Hello, || (11.into(), 21.into()))
            .unwrap()
            .admission,
        Admission::New
    );
}

#[test]
fn query_codec_rejects_unknown_versions_fields_bindings_and_invalid_evidence() {
    let authority = authority();
    let limits = ProtocolLimits::default();
    let codec = HelloClientCodec::default();
    let request = Request {
        id: 100.into(),
        authority_instance_id: 2.into(),
        selection: Selection::Operation(10.into()),
    };
    let bytes = codec.encode_query(request).unwrap();
    let response = query::respond(&authority, &bytes, &limits).unwrap();
    let decoded = codec.decode_query(&response).unwrap();
    assert!(decoded.matches(request));
    assert_eq!(decoded, authority.answer_query(request).unwrap());
    let value: serde_json::Value = serde_json::from_slice(&response).unwrap();
    for pointer in [
        "/request/protocol_version",
        "/request/extra",
        "/observation/extra",
        "/observation/phase",
        "/observation/binding/operation",
        "/observation/binding/authority",
    ] {
        let mut invalid = value.clone();
        match pointer {
            "/request/extra" => invalid["request"]["extra"] = true.into(),
            "/observation/extra" => invalid["observation"]["extra"] = true.into(),
            "/observation/phase" => *invalid.pointer_mut(pointer).unwrap() = "accepted".into(),
            "/request/protocol_version" => *invalid.pointer_mut(pointer).unwrap() = 99.into(),
            _ => {
                *invalid.pointer_mut(pointer).unwrap() =
                    "00000000-0000-0000-0000-000000000099".into()
            }
        }
        assert!(
            codec
                .decode_query(&serde_json::to_vec(&invalid).unwrap())
                .is_err(),
            "must reject {pointer}"
        );
    }
    let wrong_target = codec
        .encode_query(Request {
            authority_instance_id: 9.into(),
            ..request
        })
        .unwrap();
    assert!(
        query::respond(&authority, &wrong_target, &limits).is_err(),
        "wrong Authority must not impersonate target NotFound"
    );
    let mut invalid_request: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    invalid_request["selection"]["task"] = "hello".into();
    assert!(
        query::respond(
            &authority,
            &serde_json::to_vec(&invalid_request).unwrap(),
            &limits
        )
        .is_err()
    );
    assert!(query::respond(&authority, b"{broken", &limits).is_err());
    assert!(codec.decode_query(b"{broken").is_err());
    let request = Request {
        selection: Selection::Report(31.into()),
        ..request
    };
    let response =
        query::respond(&authority, &codec.encode_query(request).unwrap(), &limits).unwrap();
    assert_eq!(
        codec.decode_query(&response).unwrap(),
        authority.answer_query(request).unwrap()
    );
    let mut invalid: serde_json::Value = serde_json::from_slice(&response).unwrap();
    // A valid JSON envelope still must satisfy the domain evidence rules.
    invalid["observation"]["envelope"]["message"]["body"]["outcome"] = "rejected".into();
    assert!(
        codec
            .decode_query(&serde_json::to_vec(&invalid).unwrap())
            .is_err()
    );
    let request = Request {
        selection: Selection::Report(99.into()),
        ..request
    };
    let response =
        query::respond(&authority, &codec.encode_query(request).unwrap(), &limits).unwrap();
    assert_eq!(
        codec.decode_query(&response).unwrap().observation,
        Observation::NotFound
    );
    let mut invalid: serde_json::Value = serde_json::from_slice(&response).unwrap();
    invalid["observation"]["extra"] = true.into();
    assert!(
        codec
            .decode_query(&serde_json::to_vec(&invalid).unwrap())
            .is_err()
    );
    let mut tiny = ProtocolLimits::default();
    tiny.max_message_bytes = 1;
    assert!(HelloClientCodec::new(tiny).encode_query(request).is_err());
    assert!(HelloClientCodec::new(tiny).decode_query(&response).is_err());
    assert!(query::respond(&authority, &bytes, &tiny).is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn result_observation_uses_notices_or_queries_over_explicit_shared_sessions() {
    use beamlet::observe::{Budget, State};
    let authority = authority();
    let expected = authority.report_notice(30.into()).unwrap().report;
    let (server, endpoint) = listener().await;
    timeout(Duration::from_secs(20), async {
        for round in 0..2 {
            let inbox = format!("beamlet/test/observed/{round}");
            let mut connection = Transport::connect(&endpoint, &inbox, Limits::default())
                .await
                .unwrap();
            let topic = query::notice_topic(2.into());
            let notices = connection.subscribe(&topic).await.unwrap();
            let to_client = server.sender(&inbox).await.unwrap();
            let to_notices = server.sender(&topic).await.unwrap();
            to_client.wait_for_receiver().await.unwrap();
            to_notices.wait_for_receiver().await.unwrap();
            let service = async {
                let bytes = server.recv().await.unwrap();
                if round == 0 {
                    // Deliberately omit the query reply. Full retained evidence arrives as a notice.
                    let bytes = query::encode_notice(
                        authority.report_notice(30.into()).unwrap(),
                        &ProtocolLimits::default(),
                    )
                    .unwrap();
                    to_notices.send(bytes.clone()).await.unwrap();
                    to_notices.send(bytes).await.unwrap();
                } else {
                    // No notifications at all: the result existed before this subscription.
                    to_client
                        .send(
                            query::respond(&authority, &bytes, &ProtocolLimits::default()).unwrap(),
                        )
                        .await
                        .unwrap();
                    let bytes = server.recv().await.unwrap();
                    to_client
                        .send(
                            query::respond(&authority, &bytes, &ProtocolLimits::default()).unwrap(),
                        )
                        .await
                        .unwrap();
                }
            };
            let application = async {
                let channel = ClientTransport::bind(&mut connection, "beamlet/test/queries")
                    .await
                    .unwrap();
                let mut client = Client::new(channel, HelloClientCodec::default(), Some(2.into()));
                let mut state = State::new(2.into(), 10.into(), 2).unwrap();
                let mut id = 100_u128;
                let mut allocate = || {
                    id += 1;
                    id.into()
                };
                assert_eq!(
                    client
                        .observe(&mut state, &notices, Budget::default(), &mut allocate)
                        .await
                        .unwrap(),
                    Some(expected.clone())
                );
                assert_eq!(
                    client
                        .observe(&mut state, &notices, Budget::default(), &mut allocate)
                        .await
                        .unwrap(),
                    None
                );
            };
            tokio::join!(service, application);
        }
    })
    .await
    .expect("notice and fallback paths must complete with explicit subscriptions");
}

#[test]
fn result_notice_codec_validates_both_snapshot_and_evidence() {
    use beamlet::observe::Codec;
    let authority = authority();
    let codec = HelloClientCodec::default();
    let notice = authority.report_notice(31.into()).unwrap();
    let bytes = query::encode_notice(notice.clone(), &ProtocolLimits::default()).unwrap();
    assert_eq!(codec.decode_notice(&bytes).unwrap(), notice);
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    for pointer in [
        "/protocol_version",
        "/operation/phase",
        "/operation/binding/worker",
        "/report/envelope/message/body/outcome",
    ] {
        let mut invalid = value.clone();
        *invalid.pointer_mut(pointer).unwrap() = match pointer {
            "/protocol_version" => 99.into(),
            "/operation/phase" => "accepted".into(),
            "/operation/binding/worker" => "00000000-0000-0000-0000-000000000099".into(),
            _ => "rejected".into(),
        };
        assert!(
            codec
                .decode_notice(&serde_json::to_vec(&invalid).unwrap())
                .is_err(),
            "reject {pointer}"
        );
    }
    let mut invalid = value;
    invalid["extra"] = true.into();
    assert!(
        codec
            .decode_notice(&serde_json::to_vec(&invalid).unwrap())
            .is_err()
    );
    assert!(codec.decode_notice(b"{broken").is_err());
    let mut invalid = notice.clone();
    invalid.operation.adopted_report = None;
    assert!(query::encode_notice(invalid, &ProtocolLimits::default()).is_err());
    let mut tiny = ProtocolLimits::default();
    tiny.max_message_bytes = 1;
    assert!(query::encode_notice(notice, &tiny).is_err());
    assert!(HelloClientCodec::new(tiny).decode_notice(&bytes).is_err());
}
