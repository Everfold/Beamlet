//! 有界内存入口, 仅用于通过同一接口测试 Authority.

use super::{Ingress, Reply, Request};
use anyhow::Result;
use beamlet_api::{Pool, Selection, Submission, Task};
use tokio::sync::{mpsc, oneshot};

/// Authority 内部编排测试的业务请求发送端.
#[derive(Clone)]
pub(crate) struct Sender(
    /// 结构化请求的有界通道发送端.
    mpsc::Sender<Request>,
);

/// 可以停止并丢弃队列的测试业务输入.
pub(crate) struct Input(
    /// 仍开放的接收端; None 表示已停止并释放剩余请求.
    Option<mpsc::Receiver<Request>>,
);

/// 创建供 Authority 单元测试使用的有界输入通道.
///
/// # 参数
///
/// - `capacity`: 有界接收队列的容量, 必须大于零且不超过 Tokio 最大许可数.
///
/// # 返回值
///
/// 二元组 `(提交发送端, 输入接收端)`.
pub(crate) fn pair(capacity: usize) -> (Sender, Input) {
    let (send, recv) = mpsc::channel(capacity);
    (Sender(send), Input(Some(recv)))
}

impl Sender {
    /// 把测试文本转换为结构化提交并分配独立回复通道.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Sender` 的共享引用.
    /// - `text`: 提交给受控倒序处理器的原始测试文本.
    ///
    /// # 返回值
    ///
    /// 成功返回结果文本的 oneshot 接收端; 业务输入关闭时返回错误.
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
    /// 从测试通道读取下一份结构化 Authority 提交.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Input` 的可变引用.
    ///
    /// # 返回值
    ///
    /// 有请求返回 `Ok(Some(Request))`, 入口关闭且排空后返回 `Ok(None)`.
    async fn receive(&mut self) -> Result<Option<Request>, crate::RunError> {
        Ok(match &mut self.0 {
            Some(input) => input.recv().await,
            None => None,
        })
    }

    /// 停止测试输入, 丢弃尚未交出的业务请求.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Input` 的可变引用.
    ///
    /// # 返回值
    ///
    /// 返回 `Ok(())`; 有停止观察者时通知它已执行关闭.
    async fn stop(&mut self) -> Result<(), crate::RunError> {
        // 未交出的请求不属于已签发任务, 直接丢弃而不是隐式排空执行.
        self.0 = None;
        Ok(())
    }
}

/// 构造把 Authority 结果转换为测试文本的一次性回复入口.
///
/// # 参数
///
/// - `sender`: 一次性测试结果发送端.
///
/// # 返回值
///
/// Reply; 发送时完成/拒绝/未解决结果被转换为相应文本.
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
