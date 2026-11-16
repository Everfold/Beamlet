//! 有界内存入口, 仅用于通过同一接口测试 Authority.

use super::{Ingress, Reply, Request};
use anyhow::{Context, Result};
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
                text,
                reply: Reply::channel(send),
            })
            .await
            .context("请求入口已关闭")?;

        Ok(recv)
    }
}

impl Ingress for Input {
    async fn receive(&mut self) -> Result<Option<Request>> {
        Ok(match &mut self.0 {
            Some(input) => input.recv().await,
            None => None,
        })
    }

    async fn stop(&mut self) -> Result<()> {
        // 未交出的请求不属于已签发任务, 直接丢弃而不是隐式排空执行.
        self.0 = None;
        Ok(())
    }

    async fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}
