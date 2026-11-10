use beamlet::{
    Authority, Worker,
    attempt_delivery::{Event, Limits as DeliveryLimits, Outbox, Status},
    memory::Limits as LedgerLimits,
    model::{ExecutionResult, ImpactStatus, Message, SubmitReport},
};
use beamlet_message_zenoh::{Limits, Transport};
use beamlet_protocol_demo::{
    attempt_receipt::{self, AttemptAccepted},
    hello::{Dispatch, Task},
    protocol::Limits as ProtocolLimits,
};
use std::{net::TcpListener, time::Duration};
use tokio::time::{sleep, timeout};

async fn listener() -> (Transport, String) {
    for _ in 0..5 {
        let socket = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("tcp/{}", socket.local_addr().unwrap());
        drop(socket);
        if let Ok(transport) = Transport::listen(
            &endpoint,
            &attempt_receipt::inbox(2.into()),
            Limits::default(),
        )
        .await
        {
            return (transport, endpoint);
        }
    }
    panic!("test listener must bind after bounded port-selection retries");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lost_receipt_retries_identical_authorization_over_an_existing_session() {
    let (server, endpoint) = listener().await;
    let worker_transport =
        Transport::connect(&endpoint, "beamlet/test/attempts", Limits::default())
            .await
            .unwrap();
    let mut authority =
        Authority::<Task, String, String>::new(2.into(), vec![3.into()], LedgerLimits::default())
            .unwrap();
    let submitted = authority
        .submit_keyed(7.into(), Task::Hello, || (10.into(), 20.into()))
        .unwrap();
    let mut outbox = Outbox::new(2.into(), DeliveryLimits::default()).unwrap();
    let protocol = ProtocolLimits::default();
    outbox
        .retain(&submitted.dispatch, |delivery| {
            Dispatch::from_delivery(delivery)?.to_json(&protocol)
        })
        .unwrap();
    let to_worker = server.sender("beamlet/test/attempts").await.unwrap();
    let to_authority = worker_transport
        .sender(&attempt_receipt::inbox(2.into()))
        .await
        .unwrap();
    timeout(Duration::from_secs(10), async {
        to_worker.wait_for_receiver().await.unwrap();
        to_authority.wait_for_receiver().await.unwrap();
        let sender = async {
            for index in 0..2 {
                let to_worker = &to_worker;
                let Some(Event::Send(transmission)) = outbox.next_event() else {
                    panic!("expected original authorization")
                };
                let ticket = transmission.ticket();
                transmission
                    .send(|worker, bytes| async move {
                        assert_eq!(worker, 3.into());
                        to_worker.wait_for_receiver().await?;
                        to_worker.send(bytes).await
                    })
                    .await
                    .unwrap();
                assert!(outbox.sent(ticket));
                if index == 0 {
                    sleep(Duration::from_secs(1)).await;
                }
            }
            let received = server.recv_message().await.unwrap();
            let receipt = AttemptAccepted::from_json(&received.payload).unwrap();
            assert_eq!(received.key_expr, receipt.topic());
            assert!(outbox.acknowledge(receipt.binding()).unwrap());
            assert!(!outbox.acknowledge(receipt.binding()).unwrap());
            assert_eq!(
                outbox.progress(10.into()).unwrap().status,
                Status::Confirmed
            );
            assert_eq!(outbox.progress(10.into()).unwrap().sends, 2);
            assert!(outbox.next_event().is_none());
        };
        let receiver = async {
            let mut worker = Worker::<Task, String, String>::new(
                2.into(),
                vec![3.into()],
                LedgerLimits::default(),
            )
            .unwrap();
            let first = worker_transport.recv().await.unwrap();
            let decoded = Dispatch::from_json(&first, &protocol).unwrap().delivery();
            assert!(worker.receive_dispatch(decoded.clone()).unwrap().is_none());
            let report = worker
                .next_report(|attempt| async move {
                    SubmitReport {
                        report_id: 30.into(),
                        operation_id: attempt.operation.operation_id,
                        attempt_id: attempt.attempt_id,
                        impact: ImpactStatus::None,
                        result: ExecutionResult::Succeeded {
                            output: "World".to_owned(),
                        },
                    }
                })
                .await
                .unwrap()
                .unwrap();
            // The first acceptance receipt is deliberately lost, though execution has completed.
            let second = worker_transport.recv().await.unwrap();
            assert_eq!(first, second);
            let decoded = Dispatch::from_json(&second, &protocol).unwrap().delivery();
            assert_eq!(
                worker.receive_dispatch(decoded.clone()).unwrap(),
                Some(report)
            );
            let receipt = AttemptAccepted::for_dispatch(&decoded).unwrap();
            assert_eq!(receipt.binding().authority_instance_id, 2.into());
            assert_eq!(receipt.binding().worker_instance_id, 3.into());
            to_authority.send(receipt.to_json().unwrap()).await.unwrap();
            assert!(
                worker
                    .next_report(|_| async { panic!("original Attempt must execute only once") })
                    .await
                    .unwrap()
                    .is_none()
            );
            assert!(matches!(decoded.message, Message::DispatchAttempt(_)));
        };
        tokio::join!(sender, receiver);
    })
    .await
    .expect("receipt recovery must finish without a Console or Node");
}
