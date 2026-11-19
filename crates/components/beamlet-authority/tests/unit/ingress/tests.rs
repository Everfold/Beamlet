//! 通过入口与交付接口驱动真实 Authority 循环, 不启动 NATS 或读取 stdin.

use super::channel;
use crate::Concurrency;
use crate::{engine as authority, engine::Dispatch as Delivery, test_support};
use beamlet_api::{Call, DeliveryError, Pool, RoleInstance, Selection, WorkerReport};
use beamlet_primitives::{Attempt, Report};
use std::{future::Future, time::Duration};
use tokio::sync::{mpsc, oneshot};

/// 给异步测试步骤施加整体等待期限, 防止回归时永久挂起.
async fn within<F: Future>(future: F) -> F::Output {
    tokio::time::timeout(Duration::from_secs(5), future)
        .await
        .expect("入口测试未在五秒内完成")
}

/// 一次待测试驱动决定完成结果的交付.
struct Work {
    /// Authority 已选择目标并签发授权的调用.
    call: Call,
    /// 把完整报告或交付错误返回给原交付等待者的通道.
    reply: oneshot::Sender<Result<WorkerReport, DeliveryError>>,
}

impl Work {
    /// 为受控交付生成倒序结果, 并发送完整报告.
    async fn succeed(self) {
        let report_id = format!("report:{}", self.call.attempt.id());
        self.reply
            .send(Ok(test_support::report(self.call, report_id)
                .await
                .unwrap()))
            .unwrap();
    }
}

/// 通过测试工作队列挂起交付的 Dispatch 实现.
#[derive(Clone)]
struct ControlledDelivery(
    /// 让测试接收并逐一释放交付的工作发送端.
    mpsc::Sender<Work>,
);

impl Delivery for ControlledDelivery {
    /// 测试使用单个出口.
    type Route = ();
    /// 为编排测试返回预设 Worker, 跳过登记选择.
    fn select(&self, _: &Pool, _: Selection, _: &str) -> Result<(RoleInstance, ()), DeliveryError> {
        Ok((instance(), ()))
    }

    /// 把交付交给测试观察者, 由观察者决定报告或错误.
    async fn deliver(&self, call: Call, _: ()) -> Result<WorkerReport, DeliveryError> {
        let (reply, response) = oneshot::channel();
        self.0.send(Work { call, reply }).await.unwrap();
        response.await.unwrap()
    }
}

/// 创建用于暂停与释放交付的测试通道.
fn controlled_delivery() -> (ControlledDelivery, mpsc::Receiver<Work>) {
    let (send, recv) = mpsc::channel(16);
    (ControlledDelivery(send), recv)
}

/// 验证并发请求在报告乱序完成时仍对应各自的回复入口.
#[tokio::test]
async fn concurrent_requests_keep_replies_when_reports_arrive_out_of_order() {
    within(async {
        let (sender, input) = channel::pair(4);
        let (delivery, mut work) = controlled_delivery();
        let authority = tokio::spawn(authority::run(
            input,
            delivery,
            instance(),
            crate::Options::default(),
            std::future::pending::<std::io::Result<()>>(),
        ));

        let mut slow = sender.submit("Hello 世界".into()).await.unwrap();
        let fast = sender.clone().submit(" second ".into()).await.unwrap();

        let mut first = work.recv().await.unwrap();
        let mut second = work.recv().await.unwrap();
        if first.call.payload != "Hello 世界" {
            std::mem::swap(&mut first, &mut second);
        }
        assert_eq!(first.call.payload, "Hello 世界");
        assert_ne!(first.call.attempt.id(), second.call.attempt.id());
        assert_ne!(
            first.call.attempt.operation_id(),
            second.call.attempt.operation_id()
        );

        second.succeed().await;
        assert_eq!(fast.await.unwrap(), " dnoces ");
        assert!(matches!(
            slow.try_recv(),
            Err(oneshot::error::TryRecvError::Empty)
        ));

        first.succeed().await;
        assert_eq!(slow.await.unwrap(), "界世 olleH");

        let empty = sender.submit(String::new()).await.unwrap();
        work.recv().await.unwrap().succeed().await;
        assert_eq!(empty.await.unwrap(), "");

        drop(sender);
        authority.await.unwrap().unwrap();
        assert!(work.recv().await.is_none());
    })
    .await;
}

/// 验证交付错误和无效报告产生未解决回复, 不重复执行.
#[tokio::test]
async fn delivery_errors_and_invalid_reports_reply_without_retrying() {
    within(async {
        let (sender, input) = channel::pair(4);
        let (delivery, mut work) = controlled_delivery();
        let authority = tokio::spawn(authority::run(
            input,
            delivery,
            instance(),
            crate::Options::default(),
            std::future::pending::<std::io::Result<()>>(),
        ));

        for error in [
            DeliveryError::NoReceiver,
            DeliveryError::NotDelivered("full".into()),
            DeliveryError::Unknown("timeout".into()),
        ] {
            let response = sender.submit("abc".into()).await.unwrap();
            let current = work.recv().await.unwrap();
            let expected = format!("错误: {error}; 操作保持等待报告, 未重试");
            current.reply.send(Err(error)).unwrap();
            assert_eq!(response.await.unwrap(), expected);
            assert!(matches!(
                work.try_recv(),
                Err(mpsc::error::TryRecvError::Empty)
            ));
        }

        for wrong_operation in [true, false] {
            let response = sender.submit("abc".into()).await.unwrap();
            let current = work.recv().await.unwrap();
            let (operation, attempt) = current.call.attempt.into_parts();
            let association = if wrong_operation {
                Attempt::from_parts("other".into(), attempt)
            } else {
                Attempt::from_parts(operation, "stale".into())
            };
            current
                .reply
                .send(Ok(Report::new("bad".into(), association, "bad".into())))
                .unwrap();
            let response = response.await.unwrap();
            assert!(response.starts_with("错误:"));
            assert!(response.ends_with("操作保持等待报告, 未重试"));
            assert!(matches!(
                work.try_recv(),
                Err(mpsc::error::TryRecvError::Empty)
            ));
        }

        // 失败不阻塞其他新请求, 但不会为原 Operation 签发替代 Attempt.
        let response = sender.submit("next".into()).await.unwrap();
        work.recv().await.unwrap().succeed().await;
        assert_eq!(response.await.unwrap(), "txen");

        drop(sender);
        authority.await.unwrap().unwrap();
        assert!(work.recv().await.is_none());
    })
    .await;
}

/// 验证满载退出丢弃未签发请求, 并等待已签发工作结束.
#[tokio::test]
async fn shutdown_at_capacity_discards_unissued_requests_and_waits_for_issued_work() {
    for limit in [1, 3, 32] {
        within(async {
            let (sender, input) = channel::pair(limit + 1);
            let (delivery, mut work) = controlled_delivery();
            let (stop, signal) = oneshot::channel();
            let authority = tokio::spawn(authority::run(
                input,
                delivery,
                instance(),
                crate::Options {
                    concurrency: Concurrency::Limited(limit),
                    ..Default::default()
                },
                async {
                    signal.await.unwrap();
                    Ok::<_, std::io::Error>(())
                },
            ));

            let mut responses = Vec::new();
            for index in 0..limit {
                responses.push(sender.submit(format!("request-{index}")).await.unwrap());
            }
            let unissued = sender.submit("not admitted".into()).await.unwrap();

            let mut issued = Vec::new();
            for _ in 0..limit {
                issued.push(work.recv().await.unwrap());
            }
            assert!(matches!(
                work.try_recv(),
                Err(mpsc::error::TryRecvError::Empty)
            ));

            stop.send(()).unwrap();
            assert!(unissued.await.is_err());
            assert!(!authority.is_finished(), "退出必须等待已经签发的交付");
            assert!(sender.submit("after stop".into()).await.is_err());

            for current in issued.into_iter().rev() {
                current.succeed().await;
            }
            for (index, response) in responses.into_iter().enumerate() {
                assert_eq!(
                    response.await.unwrap(),
                    format!("request-{index}").chars().rev().collect::<String>()
                );
            }

            authority.await.unwrap().unwrap();
            assert!(work.recv().await.is_none(), "停止后不能签发新任务");
        })
        .await;
    }
}
/// 验证交付成功或失败都会释放 Authority 的并发准入名额.
#[tokio::test]
async fn delivery_completion_releases_admission_capacity() {
    for limit in [1, 3, 32] {
        within(async {
            let (sender, input) = channel::pair(limit + 1);
            let (delivery, mut work) = controlled_delivery();
            let authority = tokio::spawn(authority::run(
                input,
                delivery,
                instance(),
                crate::Options {
                    concurrency: Concurrency::Limited(limit),
                    ..Default::default()
                },
                std::future::pending::<std::io::Result<()>>(),
            ));

            // 先取得这一调用, 不假定并发交付任务的启动顺序.
            let mut responses = vec![sender.submit("abc".into()).await.unwrap()];
            let mut issued = vec![work.recv().await.unwrap()];
            for _ in 1..=limit {
                responses.push(sender.submit("abc".into()).await.unwrap());
            }
            for _ in 1..limit {
                issued.push(work.recv().await.unwrap());
            }
            assert!(matches!(
                work.try_recv(),
                Err(mpsc::error::TryRecvError::Empty)
            ));

            issued
                .remove(0)
                .reply
                .send(Err(DeliveryError::NoReceiver))
                .unwrap();
            assert_eq!(
                responses.remove(0).await.unwrap(),
                "错误: 当前没有匹配能力的接收者; 操作保持等待报告, 未重试"
            );

            work.recv().await.unwrap().succeed().await;
            for current in issued {
                current.succeed().await;
            }
            for response in responses {
                assert_eq!(response.await.unwrap(), "cba");
            }

            drop(sender);
            authority.await.unwrap().unwrap();
        })
        .await;
    }
}
/// 验证回复发送失败不会重试交付，也不会终止 Authority，后续请求仍可正常完成。
#[tokio::test]
async fn reply_failure_does_not_stop_authority_or_retry() {
    within(async {
        let (sender, input) = channel::pair(2);
        let (delivery, mut work) = controlled_delivery();
        let authority = tokio::spawn(authority::run(
            input,
            delivery,
            instance(),
            crate::Options::default(),
            std::future::pending::<std::io::Result<()>>(),
        ));

        let abandoned = sender.submit("abc".into()).await.unwrap();
        drop(abandoned);
        work.recv().await.unwrap().succeed().await;

        let next = sender.submit("def".into()).await.unwrap();
        work.recv().await.unwrap().succeed().await;
        assert_eq!(next.await.unwrap(), "fed");
        assert!(matches!(
            work.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));

        drop(sender);
        authority.await.unwrap().unwrap();
        assert!(work.recv().await.is_none());
    })
    .await;
}

/// 构造编排单元测试复用的固定角色实例.
fn instance() -> RoleInstance {
    RoleInstance {
        role_id: beamlet_api::Uuid::from_u128(1),
        session_id: beamlet_api::Uuid::from_u128(2),
    }
}

/// 验证无限并发 Authority 接收超过默认上限的任务并保持报告关联.
#[tokio::test]
async fn unlimited_authority_accepts_more_than_default_and_keeps_report_associations() {
    within(async {
        let total = 16 + 9;
        let (sender, input) = channel::pair(total);
        let (delivery, mut work) = controlled_delivery();
        let authority = tokio::spawn(authority::run(
            input,
            delivery,
            instance(),
            crate::Options {
                concurrency: Concurrency::Unlimited,
                ..Default::default()
            },
            std::future::pending::<std::io::Result<()>>(),
        ));
        let mut responses = Vec::new();
        for index in 0..total {
            responses.push(sender.submit(format!("request:{index}")).await.unwrap());
        }
        let mut issued = Vec::new();
        let mut operation_ids = std::collections::BTreeSet::new();
        let mut attempt_ids = std::collections::BTreeSet::new();
        // No report has completed: a hidden positive limit would stall here.
        for _ in 0..total {
            let current = work.recv().await.unwrap();
            assert!(operation_ids.insert(current.call.attempt.operation_id().clone()));
            assert!(attempt_ids.insert(current.call.attempt.id().clone()));
            issued.push(current);
        }
        drop(sender); // EOF stops new requests but must wait for all issued work.
        tokio::task::yield_now().await;
        assert!(!authority.is_finished());
        for current in issued.into_iter().rev() {
            current.succeed().await;
        }
        for (index, response) in responses.into_iter().enumerate() {
            assert_eq!(
                response.await.unwrap(),
                format!("request:{index}").chars().rev().collect::<String>()
            );
        }
        authority.await.unwrap().unwrap();
        assert!(work.recv().await.is_none());
    })
    .await;
}

/// 验证多个 Authority 共享 Worker 时仍独立计算并发名额.
#[tokio::test]
async fn authority_limits_are_independent_even_when_deliveries_share_a_worker() {
    within(async {
        let (one_sender, one_input) = channel::pair(2);
        let (two_sender, two_input) = channel::pair(2);
        let (delivery, mut work) = controlled_delivery();
        let one = tokio::spawn(authority::run(
            one_input,
            delivery.clone(),
            instance(),
            crate::Options {
                concurrency: Concurrency::Limited(1),
                ..Default::default()
            },
            std::future::pending::<std::io::Result<()>>(),
        ));
        let two_instance = RoleInstance {
            role_id: beamlet_api::Uuid::new_v4(),
            session_id: beamlet_api::Uuid::new_v4(),
        };
        let two = tokio::spawn(authority::run(
            two_input,
            delivery,
            two_instance,
            crate::Options {
                concurrency: Concurrency::Limited(2),
                ..Default::default()
            },
            std::future::pending::<std::io::Result<()>>(),
        ));
        let first_response = one_sender.submit("first".into()).await.unwrap();
        let first = work.recv().await.unwrap();
        let queued_response = one_sender.submit("queued".into()).await.unwrap();
        let second_response = two_sender.submit("second".into()).await.unwrap();
        let third_response = two_sender.submit("third".into()).await.unwrap();
        let mut other = Vec::new();
        for _ in 0..2 {
            let current = work.recv().await.unwrap();
            assert_eq!(current.call.authority, two_instance);
            assert_eq!(current.call.worker, first.call.worker);
            other.push(current);
        }
        for current in other.into_iter().rev() {
            current.succeed().await;
        }
        assert_eq!(second_response.await.unwrap(), "dnoces");
        assert_eq!(third_response.await.unwrap(), "driht");
        assert!(
            tokio::time::timeout(Duration::from_millis(40), work.recv())
                .await
                .is_err(),
            "second Authority released the first Authority's capacity"
        );
        first.succeed().await;
        assert_eq!(first_response.await.unwrap(), "tsrif");
        work.recv().await.unwrap().succeed().await;
        assert_eq!(queued_response.await.unwrap(), "deueuq");
        drop((one_sender, two_sender));
        one.await.unwrap().unwrap();
        two.await.unwrap().unwrap();
    })
    .await;
}
