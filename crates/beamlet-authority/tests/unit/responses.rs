//! 可控发送背压驱动真实 Authority, 验证其他交付、名额和停机仍可推进.
use super::*;
use super::{Dispatch as Delivery, Ingress};
use crate::Concurrency;
use crate::Options as Authority;
use crate::test_support::{self, text_reverse};
use beamlet_api::Uuid;
use beamlet_api::{Pool, Selection, Task};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

/// 给异步测试步骤施加整体等待期限，防止回归时永久挂起。
///
/// # 参数
///
/// - `future`：需要在测试期限内完成的异步步骤。
///
/// # 返回值
///
/// future 的原始输出；超过测试期限会 panic 并使测试失败。
async fn within<F: Future>(future: F) -> F::Output {
    tokio::time::timeout(Duration::from_secs(5), future)
        .await
        .expect("authority response test timed out")
}

struct Input {
    receive: mpsc::Receiver<Request>,
    stopped: Option<oneshot::Sender<()>>,
}
impl Ingress for Input {
    /// 从测试通道读取下一份结构化 Authority 提交。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Input` 的可变引用。
    ///
    /// # 返回值
    ///
    /// 有请求返回 `Ok(Some(Request))`，入口关闭且排空后返回 `Ok(None)`。
    async fn receive(&mut self) -> Result<Option<Request>, RunError> {
        Ok(self.receive.recv().await)
    }
    /// 停止测试输入，丢弃尚未交出的业务请求。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Input` 的可变引用。
    ///
    /// # 返回值
    ///
    /// 返回 `Ok(())`；有停止观察者时通知它已执行关闭。
    async fn stop(&mut self) -> Result<(), RunError> {
        self.receive.close();
        while self.receive.try_recv().is_ok() {}
        if let Some(stopped) = self.stopped.take() {
            let _ = stopped.send(());
        }
        Ok(())
    }
}
struct Work(Call, oneshot::Sender<WorkerReport>);
impl Work {
    /// 完成受控任务并把倒序结果作为报告回复。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Work` 的值；调用后转移其所有权（Copy 类型按值复制）。
    ///
    /// # 返回值
    ///
    /// `()`；测试报告或回复通道失败时 panic。
    async fn finish(self) {
        let report = test_support::report(self.0, Uuid::new_v4().to_string())
            .await
            .unwrap();
        self.1.send(report).unwrap();
    }
}
#[derive(Clone)]
struct Sender(mpsc::Sender<Work>);
impl Delivery for Sender {
    /// 按测试能力名称决定是否提供预设 Worker。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Sender` 的共享引用。
    /// - `_`：为满足测试接口签名保留的占位参数，本实现不读取该值。
    /// - `_`：为满足测试接口签名保留的占位参数，本实现不读取该值。
    /// - `capability`：要调用或匹配的能力标识，保留精确拼写。
    ///
    /// # 返回值
    ///
    /// 能力匹配时返回固定实例；不匹配返回 NoReceiver。
    fn select(
        &self,
        _: &Pool,
        _: Selection,
        capability: &str,
    ) -> Result<RoleInstance, DeliveryError> {
        if capability != text_reverse::CAPABILITY {
            return Err(DeliveryError::NoReceiver);
        }
        Ok(instance())
    }
    /// 把交付发送给测试控制端，等待它返回完整报告。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Sender` 的共享引用。
    /// - `call`：已指定 pool、Authority 和 Worker 会话的调用，包含 Attempt、能力及业务输入。
    ///
    /// # 返回值
    ///
    /// 返回 `Ok(WorkerReport)`；测试通道意外关闭时 panic。
    async fn deliver(&self, call: Call) -> Result<WorkerReport, DeliveryError> {
        let (send, receive) = oneshot::channel();
        self.0.send(Work(call, send)).await.unwrap();
        Ok(receive.await.unwrap())
    }
}
/// 构造编排单元测试复用的固定角色实例。
///
/// # 参数
///
/// 无。
///
/// # 返回值
///
/// 预设非 nil 的角色和会话身份。
fn instance() -> RoleInstance {
    RoleInstance {
        role_id: Uuid::from_u128(1),
        session_id: Uuid::from_u128(2),
    }
}
/// 为回复背压测试构造固定 payload 的 Authority 请求。
///
/// # 参数
///
/// - `reply`：发送结果的有界通道，可通过填满它来模拟回复背压。
/// - `capability`：要调用或匹配的能力标识，保留精确拼写。
///
/// # 返回值
///
/// 带指定能力和可阻塞回复通道的 Request。
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
/// 创建并预先占满一个结果通道，用于制造回复背压。
///
/// # 参数
///
/// 无。
///
/// # 返回值
///
/// 二元组 `(发送端, 接收端)`，容量已被一项拒绝结果占满。
fn blocked_reply() -> (mpsc::Sender<Outcome>, mpsc::Receiver<Outcome>) {
    let (send, receive) = mpsc::channel(1);
    send.try_send(Outcome::Rejected {
        reason: "buffer occupied".into(),
    })
    .unwrap();
    (send, receive)
}

/// 验证回复发送受阻时其他工作和停止信号仍能推进。
///
/// # 参数
///
/// 无。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
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

/// 验证待发送回复有上限，释放一个回复后恢复任务准入。
///
/// # 参数
///
/// 无。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
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
