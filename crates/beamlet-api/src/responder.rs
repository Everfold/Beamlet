use std::{error::Error, fmt, future::Future, pin::Pin};

/// 擦除具体 future 类型的一次性回复发送任务，完成时给出发送结果。
type SendFuture = Pin<Box<dyn Future<Output = Result<(), ReplyError>> + Send>>;
/// 拥有发送资源的一次性异步发送函数；消费回复值后生成 SendFuture。
///
/// # 泛型参数
///
/// - `T`：一次性回复数据类型。
type SendReply<T> = Box<dyn FnOnce(T) -> SendFuture + Send>;

/// 一次性异步回复入口, 隐藏传输地址并由适配器持有发送资源.
///
/// 丢弃入口或发送 future 不表示远端工作已取消. 发送成功仅表示适配器接受回复,
/// 不表示远端已经接受 Report 或推进了 Operation.
///
/// ```compile_fail
/// use beamlet_api::Responder;
/// async fn twice(reply: Responder<String>) {
/// reply.send(String::from("first")).await.unwrap();
/// reply.send(String::from("second")).await.unwrap();
/// }
/// ```
///
/// # 泛型参数
///
/// - `T`：一次性回复数据类型。
pub struct Responder<T> {
    /// 消费唯一回复值的发送函数，同时拥有适配器需要的发送资源。
    send: SendReply<T>,
}

impl<T> Responder<T> {
    /// 把一次性异步发送函数封装为回复入口。
    ///
    /// # 参数
    ///
    /// - `send`：消费一份回复值并异步完成发送的一次性函数；失败应返回 ReplyError。
    ///
    /// # 返回值
    ///
    /// 拥有发送函数的 Responder；构造时不执行发送。
    pub fn new<F, Fut>(send: F) -> Self
    where
        F: FnOnce(T) -> Fut + Send + 'static,
        Fut: Future<Output = Result<(), ReplyError>> + Send + 'static,
    {
        Self {
            send: Box::new(move |value| Box::pin(send(value))),
        }
    }

    /// 消费回复入口并等待唯一一次发送完成。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Responder<T>` 的值；调用后转移其所有权（Copy 类型按值复制）。
    /// - `value`：本次唯一回复的数据；所有权交给构造时注入的发送函数。
    ///
    /// # 返回值
    ///
    /// 适配器发送完成返回 `Ok(())`；失败返回 ReplyError，不进行重试。
    ///
    /// # Errors
    /// 返回适配器报告的错误, 保留其 source; 不自动重试.
    pub async fn send(self, value: T) -> Result<(), ReplyError> {
        (self.send)(value).await
    }
}

impl<T> fmt::Debug for Responder<T> {
    /// 向格式化目标写入 `Responder<T>` 的调试信息。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Responder<T>` 的共享引用。
    /// - `f`：接收格式化文本的写入目标。
    ///
    /// # 返回值
    ///
    /// 写入成功返回 `Ok(())`；格式化目标拒绝写入时返回 `fmt::Error`。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Responder").finish_non_exhaustive()
    }
}

/// 回复未完成发送, 保留适配器提供的原始原因.
#[derive(Debug)]
pub struct ReplyError {
    /// 被包装的底层错误对象，通过 Error::source 保留原因链。
    source: Box<dyn Error + Send + Sync>,
}

impl ReplyError {
    /// 包装底层失败原因并保留错误链。
    ///
    /// # 参数
    ///
    /// - `source`：底层失败原因；包装后仍可通过错误 source 链访问。
    ///
    /// # 返回值
    ///
    /// 持有原始错误的具体错误对象，不修改或重试失败操作。
    pub fn new(source: impl Into<Box<dyn Error + Send + Sync>>) -> Self {
        Self {
            source: source.into(),
        }
    }
}

impl fmt::Display for ReplyError {
    /// 向格式化目标写入 `ReplyError` 的显示文本。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `ReplyError` 的共享引用。
    /// - `f`：接收格式化文本的写入目标。
    ///
    /// # 返回值
    ///
    /// 写入成功返回 `Ok(())`；格式化目标拒绝写入时返回 `fmt::Error`。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.source.fmt(f)
    }
}

impl Error for ReplyError {
    /// 借用被包装的底层错误。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `ReplyError` 的共享引用。
    ///
    /// # 返回值
    ///
    /// 始终为 `Some` 的错误引用，可继续访问其 source 链。
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.source.as_ref())
    }
}
