use std::{error::Error, fmt, future::Future, pin::Pin};

type SendFuture = Pin<Box<dyn Future<Output = Result<(), ReplyError>> + Send>>;
type SendReply<T> = Box<dyn FnOnce(T) -> SendFuture + Send>;

/// 一次性异步回复入口, 隐藏传输地址并由适配器持有发送资源.
///
/// 丢弃入口或发送 future 不表示远端工作已取消. 发送成功仅表示适配器接受回复,
/// 不表示远端已经接受 Report 或推进了 Operation.
///
/// ```compile_fail
/// use beamlet_api::Responder;
/// async fn twice(reply: Responder<String>) {
///     reply.send(String::from("first")).await.unwrap();
///     reply.send(String::from("second")).await.unwrap();
/// }
/// ```
pub struct Responder<T> {
    send: SendReply<T>,
}

impl<T> Responder<T> {
    /// 从适配器的发送函数构造入口, 不执行发送或启动后台任务.
    pub fn new<F, Fut>(send: F) -> Self
    where
        F: FnOnce(T) -> Fut + Send + 'static,
        Fut: Future<Output = Result<(), ReplyError>> + Send + 'static,
    {
        Self {
            send: Box::new(move |value| Box::pin(send(value))),
        }
    }

    /// 消费回复入口并等待适配器完成发送.
    ///
    /// # Errors
    /// 返回适配器报告的错误, 保留其 source; 不自动重试.
    pub async fn send(self, value: T) -> Result<(), ReplyError> {
        (self.send)(value).await
    }
}

impl<T> fmt::Debug for Responder<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Responder").finish_non_exhaustive()
    }
}

/// 回复未完成发送, 保留适配器提供的原始原因.
#[derive(Debug)]
pub struct ReplyError {
    source: Box<dyn Error + Send + Sync>,
}

impl ReplyError {
    /// 封装发送失败原因, 不将其解释为远端未执行.
    pub fn new(source: impl Into<Box<dyn Error + Send + Sync>>) -> Self {
        Self {
            source: source.into(),
        }
    }
}

impl fmt::Display for ReplyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.source.fmt(f)
    }
}

impl Error for ReplyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.source.as_ref())
    }
}
