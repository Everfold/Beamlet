//! 通过入口与交付接口驱动真实 Authority 循环, 不启动 NATS 或读取 stdin.

use super::{Ingress as _, channel};
use crate::{
    authority,
    delivery::{Call, Delivery, DeliveryError, RECEIVE_CAPACITY, WorkerReport},
    worker,
};
use beamlet_primitives::{Attempt, Report};
use beamlet_protocol::{Outcome, Pool, RoleInstance, Selection};
use std::{future::Future, time::Duration};
use tokio::sync::{mpsc, oneshot};

/// 所有等待都有上限, 回归时不让测试挂住.
async fn within<F: Future>(future: F) -> F::Output {
    tokio::time::timeout(Duration::from_secs(5), future)
        .await
        .expect("入口测试未在五秒内完成")
}

struct Work {
    call: Call,
    reply: oneshot::Sender<Result<WorkerReport, DeliveryError>>,
}

impl Work {
    fn succeed(self) {
        let report_id = format!("report:{}", self.call.attempt.id());
        self.reply
            .send(Ok(worker::execute(self.call, report_id).unwrap()))
            .unwrap();
    }
}

#[derive(Clone)]
struct ControlledDelivery(mpsc::Sender<Work>);

impl Delivery for ControlledDelivery {
    fn select(&self, _: &Pool, _: Selection, _: &str) -> Result<RoleInstance, DeliveryError> {
        Ok(instance())
    }

    async fn deliver(&self, call: Call) -> Result<WorkerReport, DeliveryError> {
        let (reply, response) = oneshot::channel();
        self.0.send(Work { call, reply }).await.unwrap();
        response.await.unwrap()
    }
}

fn controlled_delivery() -> (ControlledDelivery, mpsc::Receiver<Work>) {
    let (send, recv) = mpsc::channel(RECEIVE_CAPACITY);
    (ControlledDelivery(send), recv)
}

#[tokio::test]
async fn concurrent_requests_keep_replies_when_reports_arrive_out_of_order() {
    within(async {
        let (sender, input) = channel::pair(4);
        let (delivery, mut work) = controlled_delivery();
        let authority = tokio::spawn(authority::run(
            input,
            delivery,
            instance(),
            crate::settings::Authority::default().max_concurrent_tasks,
            std::future::pending(),
        ));

        let mut slow = sender.submit("Hello 世界".into()).await.unwrap();
        let fast = sender.clone().submit(" second ".into()).await.unwrap();

        let mut first = work.recv().await.unwrap();
        let mut second = work.recv().await.unwrap();
        if first.call.text != "Hello 世界" {
            std::mem::swap(&mut first, &mut second);
        }
        assert_eq!(first.call.text, "Hello 世界");
        assert_ne!(first.call.attempt.id(), second.call.attempt.id());
        assert_ne!(
            first.call.attempt.operation_id(),
            second.call.attempt.operation_id()
        );

        second.succeed();
        assert_eq!(fast.await.unwrap(), " dnoces ");
        assert!(matches!(
            slow.try_recv(),
            Err(oneshot::error::TryRecvError::Empty)
        ));

        first.succeed();
        assert_eq!(slow.await.unwrap(), "界世 olleH");

        let empty = sender.submit(String::new()).await.unwrap();
        work.recv().await.unwrap().succeed();
        assert_eq!(empty.await.unwrap(), "");

        drop(sender);
        authority.await.unwrap().unwrap();
        assert!(work.recv().await.is_none());
    })
    .await;
}

#[tokio::test]
async fn delivery_errors_and_invalid_reports_reply_without_retrying() {
    within(async {
        let (sender, input) = channel::pair(4);
        let (delivery, mut work) = controlled_delivery();
        let authority = tokio::spawn(authority::run(
            input,
            delivery,
            instance(),
            crate::settings::Authority::default().max_concurrent_tasks,
            std::future::pending(),
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
        work.recv().await.unwrap().succeed();
        assert_eq!(response.await.unwrap(), "txen");

        drop(sender);
        authority.await.unwrap().unwrap();
        assert!(work.recv().await.is_none());
    })
    .await;
}

#[tokio::test]
async fn shutdown_at_capacity_discards_unissued_requests_and_waits_for_issued_work() {
    for limit in [1, 3, 32] {
        within(async {
            let (sender, input) = channel::pair(limit + 1);
            let (delivery, mut work) = controlled_delivery();
            let (stop, signal) = oneshot::channel();
            let authority =
                tokio::spawn(authority::run(input, delivery, instance(), limit, async {
                    signal.await.unwrap();
                    Ok(())
                }));

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
                current.succeed();
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
                limit,
                std::future::pending(),
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

            work.recv().await.unwrap().succeed();
            for current in issued {
                current.succeed();
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
#[tokio::test]
async fn channel_backpressure_cancellation_and_stopped_replies() {
    within(async {
        let (sender, mut input) = channel::pair(1);

        {
            let receive = input.receive();
            tokio::pin!(receive);
            assert!(futures::poll!(&mut receive).is_pending());
        }

        let accepted_response = sender.submit("first".into()).await.unwrap();
        let second = sender.submit("queued".into());
        tokio::pin!(second);
        assert!(futures::poll!(&mut second).is_pending(), "入口必须有界");
        let accepted = input.receive().await.unwrap().unwrap();
        assert_eq!(accepted.submission.task.text, "first");
        let queued_response = second.await.unwrap();

        input.stop().await.unwrap();
        input.stop().await.unwrap();
        assert!(input.receive().await.unwrap().is_none());
        assert!(queued_response.await.is_err());
        assert!(sender.submit("new".into()).await.is_err());

        accepted
            .reply
            .send(Outcome::Completed {
                operation_id: "operation".into(),
                attempt_id: "attempt".into(),
                report_id: "report".into(),
                worker: instance(),
                result: "result".into(),
            })
            .await
            .unwrap();
        input.flush().await.unwrap();
        assert_eq!(accepted_response.await.unwrap(), "result");
    })
    .await;
}

#[tokio::test]
async fn reply_failure_is_returned_instead_of_silently_retrying() {
    within(async {
        let (sender, input) = channel::pair(1);
        let (delivery, mut work) = controlled_delivery();
        let authority = tokio::spawn(authority::run(
            input,
            delivery,
            instance(),
            crate::settings::Authority::default().max_concurrent_tasks,
            std::future::pending(),
        ));

        let abandoned = sender.submit("abc".into()).await.unwrap();
        drop(abandoned);
        work.recv().await.unwrap().succeed();

        assert!(
            authority
                .await
                .unwrap()
                .unwrap_err()
                .to_string()
                .contains("请求回复端已关闭")
        );
        assert!(work.recv().await.is_none());
    })
    .await;
}

fn instance() -> RoleInstance {
    RoleInstance {
        role_id: uuid::Uuid::from_u128(1),
        session_id: uuid::Uuid::from_u128(2),
    }
}

#[tokio::test]
async fn missing_worker_is_rejected_before_delivery() {
    within(async {
        let (sender, input) = channel::pair(1);
        let delivery = crate::delivery::channel::Delivery::new(Duration::from_secs(1));
        let authority = tokio::spawn(authority::run(
            input,
            delivery,
            instance(),
            crate::settings::Authority::default().max_concurrent_tasks,
            std::future::pending(),
        ));
        assert_eq!(
            sender.submit("abc".into()).await.unwrap().await.unwrap(),
            "拒绝: 当前没有匹配能力的接收者"
        );
        drop(sender);
        authority.await.unwrap().unwrap();
    })
    .await;
}

#[tokio::test]
async fn unlimited_authority_accepts_more_than_default_and_keeps_report_associations() {
    within(async {
        let total = crate::settings::Authority::default().max_concurrent_tasks + 9;
        let (sender, input) = channel::pair(total);
        let (delivery, mut work) = controlled_delivery();
        let authority = tokio::spawn(authority::run(
            input,
            delivery,
            instance(),
            0,
            std::future::pending(),
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
            current.succeed();
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
            1,
            std::future::pending(),
        ));
        let two_instance = RoleInstance {
            role_id: uuid::Uuid::new_v4(),
            session_id: uuid::Uuid::new_v4(),
        };
        let two = tokio::spawn(authority::run(
            two_input,
            delivery,
            two_instance,
            2,
            std::future::pending(),
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
            current.succeed();
        }
        assert_eq!(second_response.await.unwrap(), "dnoces");
        assert_eq!(third_response.await.unwrap(), "driht");
        assert!(
            tokio::time::timeout(Duration::from_millis(40), work.recv())
                .await
                .is_err(),
            "second Authority released the first Authority's capacity"
        );
        first.succeed();
        assert_eq!(first_response.await.unwrap(), "tsrif");
        work.recv().await.unwrap().succeed();
        assert_eq!(queued_response.await.unwrap(), "deueuq");
        drop((one_sender, two_sender));
        one.await.unwrap().unwrap();
        two.await.unwrap().unwrap();
    })
    .await;
}
