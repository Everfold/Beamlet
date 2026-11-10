use beamlet::{role_observation as roles, worker_observation as workers};
use beamlet_agent::testing::{
    lifecycle::{ExitOutcome, Role, RoleNotice},
    worker_lifecycle::Announcement,
};
use beamlet_message_zenoh::{Limits, Transport};
use std::{net::TcpListener, time::Duration};
use tokio::time::timeout;

fn started() -> Announcement {
    Announcement::started(
        "00000000-0000-0000-0000-000000000001".parse().unwrap(),
        "00000000-0000-0000-0000-000000000002".parse().unwrap(),
        "00000000-0000-0000-0000-000000000003".parse().unwrap(),
        "00000000-0000-0000-0000-000000000004".parse().unwrap(),
        "00000000-0000-0000-0000-000000000005".parse().unwrap(),
    )
}

#[test]
fn worker_wire_validation_rejects_ambiguous_sequences_and_preserves_every_identity() {
    let original = started();
    for announcement in [
        original.clone(),
        original.heartbeat(1),
        original.heartbeat(u64::MAX),
    ] {
        let decoded = Announcement::from_json(&announcement.to_json().unwrap()).unwrap();
        assert_eq!(
            decoded.observation().unwrap(),
            announcement.observation().unwrap()
        );
        let binding = decoded.observation().unwrap().binding;
        assert_eq!(binding.node_id, 1.into());
        assert_eq!(binding.node_instance_id, 2.into());
        assert_eq!(binding.worker_id, 3.into());
        assert_eq!(binding.worker_instance_id, 4.into());
        assert_eq!(binding.authority_instance_id, 5.into());
    }
    for (key, value) in [
        ("sequence", serde_json::json!(1)),
        ("kind", serde_json::json!("worker_heartbeat")),
        ("kind", serde_json::json!("unknown")),
        ("protocol_version", serde_json::json!(1)),
        ("extra", serde_json::json!(true)),
    ] {
        let mut body: serde_json::Value =
            serde_json::from_slice(&original.to_json().unwrap()).unwrap();
        body[key] = value;
        assert!(Announcement::from_json(&serde_json::to_vec(&body).unwrap()).is_err());
    }
    assert!(original.heartbeat(0).to_json().is_err());
    assert!(original.heartbeat(0).observation().is_err());
    let mut body: serde_json::Value =
        serde_json::from_slice(&original.heartbeat(1).to_json().unwrap()).unwrap();
    for value in [serde_json::json!(0), serde_json::Value::Null] {
        body["sequence"] = value;
        assert!(Announcement::from_json(&serde_json::to_vec(&body).unwrap()).is_err());
    }
    assert!(Announcement::from_json(&vec![b' '; 4097]).is_err());
}

#[test]
fn role_wire_projection_preserves_kind_binding_and_both_exit_outcomes() {
    for role in [Role::Authority, Role::Worker] {
        let started = RoleNotice::new(
            role,
            "00000000-0000-0000-0000-000000000001".parse().unwrap(),
            "00000000-0000-0000-0000-000000000002".parse().unwrap(),
            "00000000-0000-0000-0000-000000000003".parse().unwrap(),
            "00000000-0000-0000-0000-000000000004".parse().unwrap(),
        );
        for notice in [
            started.clone(),
            started.exited(ExitOutcome::Drained),
            started.exited(ExitOutcome::Unconfirmed),
        ] {
            let decoded = RoleNotice::from_json(&notice.to_json().unwrap()).unwrap();
            assert_eq!(decoded.observation(), notice.observation());
            assert_eq!(decoded.observation().binding.node_id, 1.into());
            assert_eq!(decoded.observation().binding.node_instance_id, 2.into());
            assert_eq!(decoded.observation().binding.instance, 4.into());
            assert_eq!(
                decoded.observation().binding.identity,
                match role {
                    Role::Authority => roles::Identity::Authority(3.into()),
                    _ => roles::Identity::Worker(3.into()),
                }
            );
            assert_eq!(
                decoded.observation().phase,
                match notice.exit_outcome() {
                    None => roles::Phase::Started,
                    Some(ExitOutcome::Drained) => roles::Phase::Exited(roles::ExitOutcome::Drained),
                    _ => roles::Phase::Exited(roles::ExitOutcome::Unconfirmed),
                }
            );
        }
    }
}

async fn listener() -> (Transport, String) {
    for _ in 0..5 {
        let socket = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("tcp/{}", socket.local_addr().unwrap());
        drop(socket);
        if let Ok(transport) = Transport::listen(
            &endpoint,
            &beamlet_agent::testing::worker_lifecycle::inbox(
                "00000000-0000-0000-0000-000000000005".parse().unwrap(),
            ),
            Limits::default(),
        )
        .await
        {
            return (transport, endpoint);
        }
    }
    panic!("listener must bind within bounded attempts");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn existing_session_delivers_heartbeats_and_terminal_public_notices_to_library_observers() {
    let (authority, endpoint) = listener().await;
    let client = authority
        .subscribe(beamlet_agent::testing::lifecycle::SUBSCRIPTION)
        .await
        .unwrap();
    let worker = Transport::connect(&endpoint, "beamlet/test/unused", Limits::default())
        .await
        .unwrap();
    let started = started();
    let sender = worker.sender(&started.topic()).await.unwrap();
    let notice = RoleNotice::new(
        Role::Worker,
        "00000000-0000-0000-0000-000000000001".parse().unwrap(),
        "00000000-0000-0000-0000-000000000002".parse().unwrap(),
        "00000000-0000-0000-0000-000000000003".parse().unwrap(),
        "00000000-0000-0000-0000-000000000004".parse().unwrap(),
    );
    let public = worker.sender(&notice.topic()).await.unwrap();
    let mut heartbeats = workers::Observations::new(
        5.into(),
        [workers::Slot {
            instance: 4.into(),
            source: workers::Source::Remote,
        }],
    )
    .unwrap();
    let mut roles = roles::Observations::new(1).unwrap();
    timeout(Duration::from_secs(10), async {
        sender.wait_for_receiver().await.unwrap();
        public.wait_for_receiver().await.unwrap();
        for signal in [started.heartbeat(1), started.clone(), started.heartbeat(2)] {
            sender.send(signal.to_json().unwrap()).await.unwrap();
            let received = authority.recv_message().await.unwrap();
            let decoded = Announcement::from_json(&received.payload).unwrap();
            assert_eq!(received.key_expr, decoded.topic());
            heartbeats
                .record(decoded.observation().unwrap(), workers::Source::Remote)
                .unwrap();
        }
        assert_eq!(
            heartbeats
                .snapshot(4.into())
                .unwrap()
                .sequence
                .unwrap()
                .get(),
            2
        );
        assert_eq!(
            heartbeats.snapshot(4.into()).unwrap().status,
            workers::Status::Online
        );
        let exit = notice.exited(ExitOutcome::Unconfirmed);
        for (message, changed) in [(exit.clone(), true), (notice, false), (exit, false)] {
            public.send(message.to_json().unwrap()).await.unwrap();
            let received = client.recv_message().await.unwrap();
            let decoded = RoleNotice::from_json(&received.payload).unwrap();
            assert_eq!(received.key_expr, decoded.topic());
            assert_eq!(roles.record(decoded.observation()).unwrap(), changed);
        }
        assert_eq!(
            roles.snapshot(4.into()).unwrap().phase,
            roles::Phase::Exited(roles::ExitOutcome::Unconfirmed)
        );
    })
    .await
    .expect("shared-session observations must complete without Node or Console");
}
