//! 可控发送背压驱动真实 Authority, 验证其他交付、名额和停机仍可推进.
use super::*;
use crate::{ingress::Ingress, text_reverse, worker};
use beamlet_protocol::{Pool, Selection, Task};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

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
    async fn receive(&mut self) -> Result<Option<Request>> {
        Ok(self.receive.recv().await)
    }
    async fn stop(&mut self) -> Result<()> {
        self.receive.close();
        while self.receive.try_recv().is_ok() {}
        if let Some(stopped) = self.stopped.take() {
            let _ = stopped.send(());
        }
        Ok(())
    }
    async fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}
struct Work(Call, oneshot::Sender<WorkerReport>);
impl Work {
    async fn finish(self) {
        let report = worker::execute(self.0, Uuid::new_v4().to_string())
            .await
            .unwrap();
        self.1.send(report).unwrap();
    }
}
#[derive(Clone)]
struct Sender(mpsc::Sender<Work>);
impl Delivery for Sender {
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
    async fn deliver(&self, call: Call) -> Result<WorkerReport, DeliveryError> {
        let (send, receive) = oneshot::channel();
        self.0.send(Work(call, send)).await.unwrap();
        Ok(receive.await.unwrap())
    }
}
fn instance() -> RoleInstance {
    RoleInstance {
        role_id: Uuid::from_u128(1),
        session_id: Uuid::from_u128(2),
    }
}
fn request(reply: mpsc::Sender<Outcome>, capability: &str) -> Request {
    Request {
        submission: Submission {
            pool: Pool::default(),
            worker: Selection::Any,
            task: Task {
                capability: capability.into(),
                text: "abc".into(),
            },
        },
        reply: Reply::buffered(reply),
    }
}
fn blocked_reply() -> (mpsc::Sender<Outcome>, mpsc::Receiver<Outcome>) {
    let (send, receive) = mpsc::channel(1);
    send.try_send(Outcome::Rejected {
        reason: "buffer occupied".into(),
    })
    .unwrap();
    (send, receive)
}

#[tokio::test]
async fn blocked_completion_and_rejection_allow_other_work_and_shutdown() {
    within(async {
        let (send, receive) = mpsc::channel(4);
        let (stopped, observed) = oneshot::channel();
        let (work_send, mut work) = mpsc::channel(4);
        let (stop, signal) = oneshot::channel();
        let running = tokio::spawn(run(
            Input { receive, stopped: Some(stopped) }, Sender(work_send), instance(), Authority { max_concurrent_tasks: 1, ..Default::default() },
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
                    max_concurrent_tasks: 1,
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
