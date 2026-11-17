//! 有界内存入口, 仅用于通过同一接口测试 Authority.

use super::{Ingress, Reply, Request};
use anyhow::Result;
use beamlet_api::{Pool, Selection, Submission, Task};
use tokio::sync::{mpsc, oneshot};

#[derive(Clone)]
pub(crate) struct Sender(mpsc::Sender<Request>);

pub(crate) struct Input(Option<mpsc::Receiver<Request>>);

pub(crate) fn pair(capacity: usize) -> (Sender, Input) {
    let (send, recv) = mpsc::channel(capacity);
    (Sender(send), Input(Some(recv)))
}

impl Sender {
    /// 每个调用者拿到独立回复端, 不借用任何领域身份作为回复地址.
    pub(crate) async fn submit(&self, text: String) -> Result<oneshot::Receiver<String>> {
        let (send, recv) = oneshot::channel();
        self.0
            .send(Request {
                submission: Submission {
                    pool: Pool::default(),
                    worker: Selection::Any,
                    task: Task {
                        capability: crate::test_support::text_reverse::CAPABILITY.into(),
                        payload: text,
                    },
                },
                reply: reply(send),
            })
            .await
            .map_err(|_| anyhow::anyhow!("请求入口已关闭"))?;

        Ok(recv)
    }
}

impl Ingress for Input {
    async fn receive(&mut self) -> Result<Option<Request>, crate::RunError> {
        Ok(match &mut self.0 {
            Some(input) => input.recv().await,
            None => None,
        })
    }

    async fn stop(&mut self) -> Result<(), crate::RunError> {
        // 未交出的请求不属于已签发任务, 直接丢弃而不是隐式排空执行.
        self.0 = None;
        Ok(())
    }
}

fn reply(sender: oneshot::Sender<String>) -> Reply {
    Reply::new(move |outcome| async move {
        use beamlet_api::Outcome;
        let text = match outcome {
            Outcome::Completed { result, .. } => result,
            Outcome::Unresolved { reason, .. } => {
                format!("错误: {reason}; 操作保持等待报告, 未重试")
            }
            Outcome::Rejected { reason } => format!("拒绝: {reason}"),
            Outcome::Workers { .. } => {
                return Err(beamlet_api::ReplyError::new(std::io::Error::other(
                    "测试入口不接受节点查询",
                )));
            }
        };
        sender
            .send(text)
            .map_err(|_| beamlet_api::ReplyError::new(std::io::Error::other("请求回复端已关闭")))
    })
}
