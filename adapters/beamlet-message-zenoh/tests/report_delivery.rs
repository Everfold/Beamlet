use beamlet::{
    Authority, Worker,
    memory::{Limits as LedgerLimits, ReportDisposition},
    model::{ExecutionResult, ImpactStatus, Message, SubmitReport},
    report_delivery::{Event, Limits as DeliveryLimits, Outbox, Status},
};
use beamlet_message_zenoh::{Limits, Transport};
use beamlet_protocol_demo::{
    hello::{self, Task},
    protocol::Limits as ProtocolLimits,
    receipt::{self, ReportRetained},
};
use std::{net::TcpListener, time::Duration};
use tokio::time::{sleep, timeout};

async fn listener() -> (Transport, String) {
    for _ in 0..5 {
        let socket = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("tcp/{}", socket.local_addr().unwrap());
        drop(socket);
        if let Ok(transport) =
            Transport::listen(&endpoint, "beamlet/test/reports", Limits::default()).await
        {
            return (transport, endpoint);
        }
    }
    panic!("test listener must bind after bounded port-selection retries");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lost_retention_receipt_retries_original_evidence_without_reexecuting() {
    let (server, endpoint) = listener().await;
    let worker_transport =
        Transport::connect(&endpoint, &receipt::inbox(3.into()), Limits::default())
            .await
            .unwrap();
    let to_authority = worker_transport
        .sender("beamlet/test/reports")
        .await
        .unwrap();
    let to_worker = server.sender(&receipt::inbox(3.into())).await.unwrap();
    let mut authority =
        Authority::<Task, String, String>::new(2.into(), vec![3.into()], LedgerLimits::default())
            .unwrap();
    let submitted = authority
        .submit_keyed(7.into(), Task::Hello, || (10.into(), 20.into()))
        .unwrap();
    let mut worker =
        Worker::<Task, String, String>::new(2.into(), vec![3.into()], LedgerLimits::default())
            .unwrap();
    assert!(
        worker
            .receive_dispatch(submitted.dispatch.clone())
            .unwrap()
            .is_none()
    );
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
    let protocol = ProtocolLimits::default();
    let mut outbox = Outbox::new(3.into(), 2.into(), DeliveryLimits::default()).unwrap();
    outbox
        .retain(&report, |delivery| {
            hello::encode_report(delivery, &protocol)
        })
        .unwrap();
    timeout(Duration::from_secs(10), async {
        to_authority.wait_for_receiver().await.unwrap();
        to_worker.wait_for_receiver().await.unwrap();
        let sender = async {
            for number in 1..=2 {
                let Some(Event::Send(transmission)) = outbox.next_event() else {
                    panic!("expected original evidence")
                };
                let ticket = transmission.ticket();
                assert_eq!(ticket.number(), number);
                let to_authority = &to_authority;
                transmission
                    .send(|authority, bytes| async move {
                        assert_eq!(authority, 2.into());
                        to_authority.wait_for_receiver().await?;
                        to_authority.send(bytes).await
                    })
                    .await
                    .unwrap();
                assert!(outbox.sent(ticket));
                assert_eq!(outbox.unconfirmed(), 1);
                if number == 1 {
                    // A repeated Attempt returns the exact cached Report; it cannot restart delivery.
                    let cached = worker
                        .receive_dispatch(submitted.dispatch.clone())
                        .unwrap()
                        .unwrap();
                    assert_eq!(cached, report);
                    outbox
                        .retain(&cached, |_| -> Result<Vec<u8>, std::convert::Infallible> {
                            panic!("duplicate authorization cannot re-encode its cached Report")
                        })
                        .unwrap();
                    sleep(Duration::from_secs(1)).await;
                }
            }
            let received = worker_transport.recv_message().await.unwrap();
            let receipt = ReportRetained::from_json(&received.payload).unwrap();
            assert_eq!(received.key_expr, receipt.topic());
            assert!(outbox.acknowledge(receipt.binding()).unwrap());
            assert!(!outbox.acknowledge(receipt.binding()).unwrap());
            assert_eq!(
                outbox.progress(30.into()).unwrap().status,
                Status::Confirmed
            );
            assert_eq!(outbox.progress(30.into()).unwrap().sends, 2);
            assert!(outbox.next_event().is_none());
            assert_eq!(outbox.unconfirmed(), 0);
            assert!(
                worker
                    .next_report(|_| async { panic!("execution must not repeat") })
                    .await
                    .unwrap()
                    .is_none()
            );
        };
        let receiver = async {
            let first = server.recv().await.unwrap();
            let decoded = hello::decode_report(&first, &protocol).unwrap();
            assert_eq!(
                authority.receive_report(decoded.clone()).unwrap(),
                ReportDisposition::Adopted
            );
            let receipt = ReportRetained::for_report(&decoded).unwrap();
            assert_eq!(receipt.binding().execution.authority_instance_id, 2.into());
            assert_eq!(receipt.binding().execution.worker_instance_id, 3.into());
            assert_eq!(receipt.binding().report_id, 30.into());
            // The first receipt is deliberately lost after successful retention.
            let second = server.recv().await.unwrap();
            assert_eq!(first, second);
            let decoded = hello::decode_report(&second, &protocol).unwrap();
            assert_eq!(
                authority.receive_report(decoded.clone()).unwrap(),
                ReportDisposition::Duplicate
            );
            assert_eq!(authority.report(30.into()), Some(&decoded));
            to_worker.send(receipt.to_json().unwrap()).await.unwrap();
        };
        tokio::join!(sender, receiver);
        // Additional evidence is retained and acknowledged without replacing the adopted Report.
        let mut additional = report.clone();
        let Message::SubmitReport(evidence) = &mut additional.message else {
            panic!("expected evidence")
        };
        evidence.report_id = 31.into();
        evidence.impact = ImpactStatus::Observed;
        outbox
            .retain(&additional, |delivery| {
                hello::encode_report(delivery, &protocol)
            })
            .unwrap();
        let Some(Event::Send(transmission)) = outbox.next_event() else {
            panic!("additional evidence needs its own transmission")
        };
        let ticket = transmission.ticket();
        transmission
            .send(|_, bytes| async { to_authority.send(bytes).await })
            .await
            .unwrap();
        assert!(outbox.sent(ticket));
        let received = server.recv().await.unwrap();
        let decoded = hello::decode_report(&received, &protocol).unwrap();
        assert_eq!(decoded, additional);
        assert_eq!(
            authority.receive_report(decoded.clone()).unwrap(),
            ReportDisposition::Additional
        );
        let receipt = ReportRetained::for_report(&decoded).unwrap();
        to_worker.send(receipt.to_json().unwrap()).await.unwrap();
        let received = worker_transport.recv_message().await.unwrap();
        let receipt = ReportRetained::from_json(&received.payload).unwrap();
        assert_eq!(received.key_expr, receipt.topic());
        assert!(outbox.acknowledge(receipt.binding()).unwrap());
        assert_eq!(outbox.unconfirmed(), 0);
        assert_eq!(
            authority.operation(10.into()).unwrap().adopted_report,
            Some(30.into())
        );
    })
    .await
    .expect("receipt recovery must finish without Node or Console");
}
