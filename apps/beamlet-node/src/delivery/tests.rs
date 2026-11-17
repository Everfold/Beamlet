use super::{Call, Delivery as _, DeliveryError, Inbox as _, Incoming, channel};
use crate::{text_reverse::CAPABILITY as REVERSE, worker};
use beamlet_api::{Pool, RoleInstance, Selection};
use beamlet_primitives::{Attempt, Report};
use std::time::Duration;
use tokio::time::timeout;

const DEADLINE: Duration = Duration::from_millis(300);

fn call(operation: &str, capability: &str, text: &str) -> Call {
    Call {
        pool: beamlet_api::Pool::default(),
        authority: beamlet_api::RoleInstance {
            role_id: uuid::Uuid::from_u128(1),
            session_id: uuid::Uuid::from_u128(2),
        },
        worker: beamlet_api::RoleInstance {
            role_id: uuid::Uuid::from_u128(3),
            session_id: uuid::Uuid::from_u128(4),
        },
        attempt: Attempt::from_parts(operation.into(), format!("{operation}:attempt")),
        capability: capability.into(),
        payload: text.into(),
    }
}

/// 两个合格 Worker 谁先收到都合法, 但必须确有一个收到.
async fn selected(a: &mut channel::Inbox, b: &mut channel::Inbox) -> Incoming {
    timeout(Duration::from_secs(2), async {
        tokio::select! {
            incoming = a.receive() => incoming.unwrap().unwrap(),
            incoming = b.receive() => incoming.unwrap().unwrap(),
        }
    })
    .await
    .expect("匹配的 Worker 没有收到调用")
}

/// 没有第二份调用, 也没有向能力不符的 Worker 广播.
async fn assert_empty(a: &mut channel::Inbox, b: &mut channel::Inbox, c: &mut channel::Inbox) {
    assert!(
        timeout(Duration::from_millis(40), async {
            tokio::select! {
                _ = a.receive() => {},
                _ = b.receive() => {},
                _ = c.receive() => {},
            }
        })
        .await
        .is_err(),
        "额外的 Worker 收到了调用"
    );
}

/// channel 是可执行的契约参照, 不是仅模拟 NATS 返回值的 mock.
#[tokio::test]
async fn channel_contract() {
    let mut sender = channel::Delivery::new(DEADLINE);
    let mut a = sender.register(&[REVERSE], 4);
    let mut b = sender.register(&[REVERSE, "text.uppercase/v1"], 4);
    let mut c = sender.register(&["image.resize/v1"], 4);

    assert_eq!(
        sender
            .deliver(call("unsupported", "text.reverse/v2", "x"))
            .await
            .unwrap_err(),
        DeliveryError::NoReceiver
    );
    assert!(matches!(
        sender.deliver(call("invalid", "", "x")).await,
        Err(DeliveryError::NotDelivered(_))
    ));

    let first_sender = sender.clone();
    let first = tokio::spawn(async move {
        first_sender
            .deliver(call("authority-a:1", REVERSE, "Hello 世界\n🙂"))
            .await
    });
    let first_call = selected(&mut a, &mut b).await;

    let second_sender = sender.clone();
    let second = tokio::spawn(async move {
        second_sender
            .deliver(call("authority-b:1", REVERSE, ""))
            .await
    });
    let second_call = selected(&mut a, &mut b).await;
    assert_empty(&mut a, &mut b, &mut c).await;

    second_call
        .reply
        .send(
            worker::execute(second_call.call, "report-b".into())
                .await
                .unwrap(),
        )
        .await
        .unwrap();
    let report = second.await.unwrap().unwrap();
    assert_eq!(report.operation_id(), "authority-b:1");
    assert_eq!(report.attempt_id(), "authority-b:1:attempt");
    assert_eq!(report.id(), "report-b");
    assert_eq!(report.result(), "");

    first_call
        .reply
        .send(
            worker::execute(first_call.call, "report-a".into())
                .await
                .unwrap(),
        )
        .await
        .unwrap();
    let report = first.await.unwrap().unwrap();
    assert_eq!(report.operation_id(), "authority-a:1");
    assert_eq!(report.attempt_id(), "authority-a:1:attempt");
    assert_eq!(report.id(), "report-a");
    assert_eq!(report.result(), "🙂\n界世 olleH");

    let lost_sender = sender.clone();
    let lost = tokio::spawn(async move { lost_sender.deliver(call("lost", REVERSE, "x")).await });
    let late = selected(&mut a, &mut b).await;
    assert!(matches!(
        lost.await.unwrap(),
        Err(DeliveryError::Unknown(_))
    ));

    // 超时不撤销已接收的调用, 迟到报告可以丢失, 但不能触发再次交付.
    let _ = late
        .reply
        .send(worker::execute(late.call, "late".into()).await.unwrap())
        .await;
    assert_empty(&mut a, &mut b, &mut c).await;

    let wrong_sender = sender.clone();
    let wrong =
        tokio::spawn(async move { wrong_sender.deliver(call("wrong", REVERSE, "x")).await });
    let incoming = selected(&mut a, &mut b).await;
    incoming
        .reply
        .send(Report::new(
            "wrong".into(),
            Attempt::from_parts("other".into(), "other".into()),
            "bad".into(),
        ))
        .await
        .unwrap();
    assert!(matches!(
        wrong.await.unwrap(),
        Err(DeliveryError::Unknown(_))
    ));

    let dropped_sender = sender.clone();
    let dropped =
        tokio::spawn(async move { dropped_sender.deliver(call("dropped", REVERSE, "x")).await });
    drop(selected(&mut a, &mut b).await);
    assert!(matches!(
        dropped.await.unwrap(),
        Err(DeliveryError::Unknown(_))
    ));

    a.stop().await.unwrap();
    b.stop().await.unwrap();
    assert!(
        timeout(Duration::from_secs(2), a.receive())
            .await
            .unwrap()
            .unwrap()
            .is_none()
    );
    assert!(
        timeout(Duration::from_secs(2), b.receive())
            .await
            .unwrap()
            .unwrap()
            .is_none()
    );
    assert_eq!(
        sender
            .deliver(call("offline", REVERSE, "x"))
            .await
            .unwrap_err(),
        DeliveryError::NoReceiver
    );
    c.stop().await.unwrap();
}

#[tokio::test]
async fn channel_backpressure_and_worker_drain() {
    let mut sender = channel::Delivery::new(Duration::from_secs(2));
    let mut inbox = sender.register(&[REVERSE], 1);

    let first = sender.deliver(call("queued", REVERSE, "abc"));
    tokio::pin!(first);
    assert!(futures::poll!(&mut first).is_pending());
    assert!(matches!(
        sender.deliver(call("full", REVERSE, "x")).await,
        Err(DeliveryError::NotDelivered(_))
    ));

    inbox.stop().await.unwrap();
    let worker = tokio::spawn(worker::run(
        inbox,
        "worker".into(),
        1,
        std::future::pending(),
    ));
    assert_eq!(first.await.unwrap().result(), "cba");
    worker.await.unwrap().unwrap();

    assert_eq!(
        sender
            .deliver(call("closed", REVERSE, "x"))
            .await
            .unwrap_err(),
        DeliveryError::NoReceiver
    );
}

#[tokio::test]
async fn exact_targets_do_not_fall_back_across_pools_or_sessions() {
    use uuid::Uuid;
    let mut sender = channel::Delivery::new(DEADLINE);
    let first = RoleInstance {
        role_id: Uuid::new_v4(),
        session_id: Uuid::new_v4(),
    };
    let second = RoleInstance {
        session_id: Uuid::new_v4(),
        ..first
    };
    let pool: Pool = "other".parse().unwrap();
    let mut a = sender.register_in(Pool::default(), first, &[REVERSE], 2);
    let mut b = sender.register_in(Pool::default(), second, &[REVERSE], 2);
    let mut c = sender.register_in(pool.clone(), first, &[REVERSE], 2);
    assert_eq!(
        sender.select(&pool, Selection::Any, REVERSE).unwrap(),
        first
    );
    assert_eq!(
        sender.select(&pool, Selection::Exact(second), REVERSE),
        Err(DeliveryError::NoReceiver)
    );
    let mut request = call("exact", REVERSE, "abc");
    request.worker = second;
    let delivery = sender.clone();
    let pending = tokio::spawn(async move { delivery.deliver(request).await });
    let received = b.receive().await.unwrap().unwrap();
    assert_eq!(received.call.worker, second);
    received
        .reply
        .send(worker::execute(received.call, "r".into()).await.unwrap())
        .await
        .unwrap();
    assert_eq!(pending.await.unwrap().unwrap().result(), "cba");
    b.stop().await.unwrap();
    let mut stale = call("stale", REVERSE, "abc");
    stale.worker = second;
    assert_eq!(
        sender.deliver(stale).await.unwrap_err(),
        DeliveryError::NoReceiver
    );
    assert!(
        timeout(Duration::from_millis(20), a.receive())
            .await
            .is_err()
    );
    assert!(
        timeout(Duration::from_millis(20), c.receive())
            .await
            .is_err()
    );
}
