//! 可控发送背压驱动真实 Authority, 验证其他交付, 名额和停机仍可推进.
use super::*;
use super::{Dispatch as Delivery, Ingress};
use crate::Concurrency;
use crate::Options as Authority;
use crate::test_support::{self, text_reverse};
use beamlet_api::Uuid;
use beamlet_api::{Pool, Selection, Task};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

/// 给异步测试步骤施加整体等待期限, 防止回归时永久挂起.
async fn within<F: Future>(future: F) -> F::Output {
    tokio::time::timeout(Duration::from_secs(5), future)
        .await
        .expect("authority response test timed out")
}

/// 用于回复背压测试的 Authority 输入, 并可通知停止发生.
struct Input {
    /// 等待业务编排取出的请求队列.
    receive: mpsc::Receiver<Request>,
    /// 输入停止后通知测试观察者的一次性发送端.
    stopped: Option<oneshot::Sender<()>>,
}
impl Ingress for Input {
    /// 从测试通道读取下一份结构化 Authority 提交.
    async fn receive(&mut self) -> Result<Option<Request>, RunError> {
        Ok(self.receive.recv().await)
    }
    /// 停止测试输入, 丢弃尚未交出的业务请求.
    async fn stop(&mut self) -> Result<(), RunError> {
        self.receive.close();
        while self.receive.try_recv().is_ok() {}
        if let Some(stopped) = self.stopped.take() {
            let _ = stopped.send(());
        }
        Ok(())
    }
}
/// 回复背压测试中待完成的调用及其报告返回通道.
struct Work(
    /// 已经签发并指向测试 Worker 的调用.
    Call,
    /// 向原交付等待者返回完整报告的一次性通道.
    oneshot::Sender<WorkerReport>,
);
impl Work {
    /// 完成受控任务并把倒序结果作为报告回复.
    async fn finish(self) {
        let report = test_support::report(self.0, Uuid::new_v4().to_string())
            .await
            .unwrap();
        self.1.send(report).unwrap();
    }
}
/// 按测试能力选择 Worker, 并将交付交给测试驱动的发送端.
#[derive(Clone)]
struct Sender(
    /// 待完成工作项的有界发送端.
    mpsc::Sender<Work>,
);
impl Delivery for Sender {
    /// 测试使用单个出口.
    type Route = ();
    /// 按测试能力名称决定是否提供预设 Worker.
    fn select(
        &self,
        _: &Pool,
        _: Selection,
        capability: &str,
    ) -> Result<(RoleInstance, ()), DeliveryError> {
        if capability != text_reverse::CAPABILITY {
            return Err(DeliveryError::NoReceiver);
        }
        Ok((instance(), ()))
    }
    /// 把交付发送给测试控制端, 等待它返回完整报告.
    async fn deliver(&self, call: Call, _: ()) -> Result<WorkerReport, DeliveryError> {
        let (send, receive) = oneshot::channel();
        self.0.send(Work(call, send)).await.unwrap();
        Ok(receive.await.unwrap())
    }
}
/// 构造编排单元测试复用的固定角色实例.
fn instance() -> RoleInstance {
    RoleInstance {
        role_id: Uuid::from_u128(1),
        session_id: Uuid::from_u128(2),
    }
}
/// 为回复背压测试构造固定 payload 的 Authority 请求.
fn request(reply: mpsc::Sender<Outcome>, capability: &str) -> Request {
    Request {
        submission: Submission {
            pool: Pool::default(),
            worker: Selection::Any,
            task: Task {
                capability: capability.into(),
                payload: "abc".into(),
            },
        },
        reply: Reply::new(move |outcome| async move {
            reply
                .send(outcome)
                .await
                .map_err(beamlet_api::ReplyError::new)
        }),
    }
}
/// 创建并预先占满一个结果通道, 用于制造回复背压.
fn blocked_reply() -> (mpsc::Sender<Outcome>, mpsc::Receiver<Outcome>) {
    let (send, receive) = mpsc::channel(1);
    send.try_send(Outcome::Rejected {
        reason: "buffer occupied".into(),
    })
    .unwrap();
    (send, receive)
}

/// 验证回复发送受阻时其他工作和停止信号仍能推进.
#[tokio::test]
async fn blocked_completion_and_rejection_allow_other_work_and_shutdown() {
    within(async {
        let (send, receive) = mpsc::channel(4);
        let (stopped, observed) = oneshot::channel();
        let (work_send, mut work) = mpsc::channel(4);
        let (stop, signal) = oneshot::channel();
        let running = tokio::spawn(run(
            Input { receive, stopped: Some(stopped) }, Sender(work_send), instance(), Authority { concurrency: Concurrency::Limited(1), ..Default::default() },
            async { signal.await.map_err(std::io::Error::other) },
        ));
        let (rejected, mut rejection) = blocked_reply();
        send.send(request(rejected, "missing/v1")).await.unwrap();
        let (slow, mut slow_reply) = blocked_reply();
        send.send(request(slow, text_reverse::CAPABILITY)).await.unwrap();
        work.recv().await.unwrap().finish().await;
        let (fast, mut fast_reply) = mpsc::channel(1);
        send.send(request(fast, text_reverse::CAPABILITY)).await.unwrap();
        work.recv().await.unwrap().finish().await;
        assert!(matches!(fast_reply.recv().await, Some(Outcome::Completed { result, .. }) if result == "cba"));
        stop.send(()).unwrap();
        observed.await.unwrap();
        assert!(!running.is_finished(), "shutdown must wait for pending replies");
        rejection.recv().await.unwrap();
        assert!(matches!(rejection.recv().await, Some(Outcome::Rejected { .. })));
        slow_reply.recv().await.unwrap();
        assert!(matches!(slow_reply.recv().await, Some(Outcome::Completed { result, .. }) if result == "cba"));
        running.await.unwrap().unwrap();
    }).await;
}

/// 验证待发送回复有上限, 释放一个回复后恢复任务准入.
#[tokio::test]
async fn reply_backlog_is_bounded_and_releasing_one_reply_resumes_admission() {
    for limit in [1, 3, 32] {
        within(async {
            let (send, receive) = mpsc::channel(2);
            let (stopped, observed) = oneshot::channel();
            let (work_send, mut work) = mpsc::channel(2);
            let (stop, signal) = oneshot::channel();
            let running = tokio::spawn(run(
                Input {
                    receive,
                    stopped: Some(stopped),
                },
                Sender(work_send),
                instance(),
                Authority {
                    concurrency: Concurrency::Limited(1),
                    max_concurrent_replies: limit,
                    ..Default::default()
                },
                async { signal.await.map_err(std::io::Error::other) },
            ));
            let mut replies = Vec::new();
            for _ in 0..limit {
                let (reply, receive) = blocked_reply();
                replies.push(receive);
                send.send(request(reply, text_reverse::CAPABILITY))
                    .await
                    .unwrap();
                work.recv().await.unwrap().finish().await;
            }
            let (last, mut last_reply) = mpsc::channel(1);
            send.send(request(last, text_reverse::CAPABILITY))
                .await
                .unwrap();
            assert!(
                tokio::time::timeout(Duration::from_millis(40), work.recv())
                    .await
                    .is_err(),
                "full reply backlog must stop admission"
            );
            let mut released = replies.pop().unwrap();
            released.recv().await.unwrap();
            assert!(matches!(
                released.recv().await,
                Some(Outcome::Completed { .. })
            ));
            let last = work.recv().await.unwrap();
            stop.send(()).unwrap();
            observed.await.unwrap();
            assert!(!running.is_finished());
            last.finish().await;
            assert!(matches!(
                last_reply.recv().await,
                Some(Outcome::Completed { .. })
            ));
            for mut reply in replies {
                reply.recv().await.unwrap();
                assert!(matches!(
                    reply.recv().await,
                    Some(Outcome::Completed { .. })
                ));
            }
            running.await.unwrap().unwrap();
        })
        .await;
    }
}
