use beamlet_api::{
    AuthorityEvent, AuthorityInbox, AuthorityInfo, Call, ClientTransport, Command, Delivery,
    DeliveryError, Outcome, Pool, Reply, RoleInstance, Selection, Submission, Task, Uuid,
    WorkerBinding, WorkerInbox,
};
use beamlet_primitives::Attempt;
use beamlet_transport_local::Local;
use std::{
    future::{Future, poll_fn},
    task::Poll,
    time::Duration,
};

fn instance() -> RoleInstance {
    RoleInstance {
        role_id: Uuid::new_v4(),
        session_id: Uuid::new_v4(),
    }
}
fn binding() -> WorkerBinding {
    WorkerBinding {
        pool: Pool::default(),
        instance: instance(),
        capabilities: ["sum/v1".into()].into(),
    }
}
fn call(worker: RoleInstance, index: u8) -> Call {
    Call {
        pool: Pool::default(),
        authority: instance(),
        worker,
        attempt: Attempt::from_parts(format!("o{index}"), format!("a{index}")),
        capability: "sum/v1".into(),
        payload: "data".into(),
    }
}

#[tokio::test]
async fn full_queue_is_not_delivered_and_stop_drains_accepted_work() {
    let local: Local = Local::new(1).unwrap();
    let binding = binding();
    let (mut inbox, _) = local.bind_worker(binding.clone()).unwrap();
    let request = local.deliver(call(binding.instance, 1), Duration::from_secs(1));
    tokio::pin!(request);
    // 手动推进到等待回复, 确保已入队, 不依赖调度时间.
    poll_fn(|cx| {
        assert!(request.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    assert!(matches!(
        local
            .deliver(call(binding.instance, 2), Duration::from_secs(1))
            .await,
        Err(DeliveryError::NotDelivered(_))
    ));
    inbox.stop().await.unwrap();
    assert_eq!(
        local
            .deliver(call(binding.instance, 3), Duration::from_secs(1))
            .await
            .unwrap_err(),
        DeliveryError::NoReceiver
    );
    let incoming = inbox.receive().await.unwrap().unwrap();
    assert_eq!(incoming.call.attempt.id(), "a1");
    assert!(inbox.receive().await.unwrap().is_none());
    drop(incoming);
    assert!(matches!(request.await, Err(DeliveryError::Unknown(_))));
}

#[test]
fn duplicate_binding_never_overwrites_and_drop_releases_all_pools() {
    let local: Local = Local::new(2).unwrap();
    let info = AuthorityInfo {
        instance: instance(),
        pools: [Pool::default(), "extra".parse().unwrap()].into(),
        label: None,
    };
    let endpoint = local.bind_authority(&info).unwrap();
    assert!(local.bind_authority(&info).is_err());
    drop(endpoint);
    let _rebound = local.bind_authority(&info).unwrap();
    let binding = binding();
    let (endpoint, _presence) = local.bind_worker(binding.clone()).unwrap();
    assert!(local.bind_worker(binding.clone()).is_err());
    drop(endpoint);
    assert!(local.bind_worker(binding).is_ok());
    assert!(Local::<String, String>::new(0).is_err());
    assert!(Local::<String, String>::new(usize::MAX).is_err());
}

#[tokio::test]
async fn commands_and_outcomes_move_custom_data_without_clone_or_serialization() {
    struct Payload(Vec<u8>);
    struct Value(usize);
    let local = Local::<Payload, Value>::new(2).unwrap();
    let info = AuthorityInfo {
        instance: instance(),
        pools: [Pool::default()].into(),
        label: None,
    };
    let mut authority = local.bind_authority(&info).unwrap();
    let request_id = Uuid::new_v4();
    let command = Command::Submit {
        request_id,
        authority: info.instance,
        submission: Submission {
            pool: Pool::default(),
            worker: Selection::Any,
            task: Task {
                capability: "sum/v1".into(),
                payload: Payload(vec![1, 2, 3]),
            },
        },
    };
    let (result, ()) = tokio::join!(local.request(command, Duration::from_secs(1)), async {
        let AuthorityEvent::Request {
            command: Command::Submit { submission, .. },
            reply,
            ..
        } = authority.receive().await.unwrap().unwrap()
        else {
            panic!("submit expected");
        };
        reply
            .send(Reply {
                request_id,
                authority: info.instance,
                outcome: Outcome::Completed {
                    operation_id: "o".into(),
                    attempt_id: "a".into(),
                    report_id: "r".into(),
                    worker: instance(),
                    result: Value(submission.task.payload.0.len()),
                },
            })
            .await
            .unwrap();
    });
    let response = result.unwrap();
    assert_eq!(response.request_id, request_id);
    let Outcome::Completed { result, .. } = response.outcome else {
        panic!("completion expected");
    };
    assert_eq!(result.0, 3);
}
