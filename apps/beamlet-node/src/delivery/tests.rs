//! 两种适配器共用行为场景, 测试不依赖队列组的选择顺序.
use super::{
    Call, Delivery as _, DeliveryError, Inbox as _, Incoming, REVERSE, WorkerReport, channel, nats,
};
use crate::{test_support::Broker, worker};
use beamlet_primitives::{Attempt, Report};
use std::time::Duration;
use tokio::time::timeout;

const DEADLINE: Duration = Duration::from_millis(300);

#[derive(Clone)]
enum Sender {
    Channel(channel::Delivery),
    Nats(nats::Delivery),
}

impl super::Delivery for Sender {
    /// 让同一测试通过交付接口驱动两个适配器.
    async fn deliver(&self, call: Call) -> Result<WorkerReport, DeliveryError> {
        match self {
            Self::Channel(sender) => sender.deliver(call).await,
            Self::Nats(sender) => sender.deliver(call).await,
        }
    }
}

enum Inbox {
    Channel(channel::Inbox),
    Nats(nats::Inbox),
}

impl super::Inbox for Inbox {
    /// 不让共用测试观察底层传输对象.
    async fn receive(&mut self) -> anyhow::Result<Option<Incoming>> {
        match self {
            Self::Channel(inbox) => inbox.receive().await,
            Self::Nats(inbox) => inbox.receive().await,
        }
    }

    /// 验证两种实现都能停止接收并排空已有调用.
    async fn stop(&mut self) -> anyhow::Result<()> {
        match self {
            Self::Channel(inbox) => inbox.stop().await,
            Self::Nats(inbox) => inbox.stop().await,
        }
    }
}

enum Fixture {
    Channel(channel::Delivery),
    Nats(async_nats::Client),
}

impl Fixture {
    /// Worker 能力在测试开始时登记, 不使用全局名册.
    async fn register(&mut self, capabilities: &[&str]) -> Inbox {
        match self {
            Self::Channel(sender) => Inbox::Channel(sender.register(capabilities, 4)),
            Self::Nats(client) => Inbox::Nats(
                nats::Inbox::subscribe(client.clone(), capabilities)
                    .await
                    .unwrap(),
            ),
        }
    }

    /// 独立的发送句柄代表不同 Authority, 共享能力路由而非回复地址.
    fn sender(&self) -> Sender {
        match self {
            Self::Channel(sender) => Sender::Channel(sender.clone()),
            Self::Nats(client) => Sender::Nats(nats::Delivery::new(client.clone(), DEADLINE)),
        }
    }
}

/// 测试身份显式提供, 不依赖 node 的身份生成器.
fn call(operation: &str, capability: &str, text: &str) -> Call {
    Call {
        attempt: Attempt::from_parts(operation.into(), format!("{operation}:attempt")),
        capability: capability.into(),
        text: text.into(),
    }
}

/// 两个合格 Worker 谁先收到都合法, 但必须确有一个收到.
async fn selected(a: &mut Inbox, b: &mut Inbox) -> Incoming {
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
async fn assert_empty(a: &mut Inbox, b: &mut Inbox, c: &mut Inbox) {
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

/// 单选、版本隔离、乱序报告、丢报告和停接收使用完全相同的断言.
async fn contract(mut fixture: Fixture) {
    let mut a = fixture.register(&[REVERSE]).await;
    let mut b = fixture.register(&[REVERSE, "text.uppercase/v1"]).await;
    let mut c = fixture.register(&["image.resize/v1"]).await;
    let sender = fixture.sender();
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

    let first_sender = fixture.sender();
    let first = tokio::spawn(async move {
        first_sender
            .deliver(call("authority-a:1", REVERSE, "Hello 世界\n🙂"))
            .await
    });
    let first_call = selected(&mut a, &mut b).await;
    let second_sender = fixture.sender();
    let second = tokio::spawn(async move {
        second_sender
            .deliver(call("authority-b:1", REVERSE, ""))
            .await
    });
    let second_call = selected(&mut a, &mut b).await;
    assert_empty(&mut a, &mut b, &mut c).await;

    second_call
        .reply
        .send(worker::execute(second_call.call, "report-b".into()).unwrap())
        .await
        .unwrap();
    let report = second.await.unwrap().unwrap();
    assert_eq!(report.operation_id(), "authority-b:1");
    assert_eq!(report.attempt_id(), "authority-b:1:attempt");
    assert_eq!(report.id(), "report-b");
    assert_eq!(report.result(), "");
    first_call
        .reply
        .send(worker::execute(first_call.call, "report-a".into()).unwrap())
        .await
        .unwrap();
    let report = first.await.unwrap().unwrap();
    assert_eq!(report.operation_id(), "authority-a:1");
    assert_eq!(report.attempt_id(), "authority-a:1:attempt");
    assert_eq!(report.id(), "report-a");
    assert_eq!(report.result(), "🙂\n界世 olleH");

    let lost_sender = fixture.sender();
    let lost = tokio::spawn(async move { lost_sender.deliver(call("lost", REVERSE, "x")).await });
    let late = selected(&mut a, &mut b).await;
    assert!(matches!(
        lost.await.unwrap(),
        Err(DeliveryError::Unknown(_))
    ));
    // 超时不撤销已接收的调用, 迟到报告可以丢失, 但不能触发再次交付.
    let _ = late
        .reply
        .send(worker::execute(late.call, "late".into()).unwrap())
        .await;
    assert_empty(&mut a, &mut b, &mut c).await;

    let wrong_sender = fixture.sender();
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

    let dropped_sender = fixture.sender();
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

/// channel 是可执行的契约参照, 不是仅模拟 NATS 返回值的 mock.
#[tokio::test]
async fn channel_contract() {
    contract(Fixture::Channel(channel::Delivery::new(DEADLINE))).await;
}

/// 与 channel 使用同一套场景, 需显式运行带外部进程的测试.
#[tokio::test]
#[ignore = "需要 nats-server: cargo test -p beamlet-node nats_contract -- --ignored"]
async fn nats_contract() {
    let (_broker, client) = Broker::start().await;
    contract(Fixture::Nats(client)).await;
}

/// 队列满必须明确未交付, 关闭输入后 Worker 仍能完成已经入队的工作.
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
    let worker = tokio::spawn(worker::run(inbox, "worker".into(), std::future::pending()));
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
