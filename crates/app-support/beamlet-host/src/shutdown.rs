use std::convert::Infallible;
use tokio::sync::watch;

/// 只读停止通知; 所有克隆都能观察到停止, 晚开始等待也不会错过.
#[derive(Debug, Clone)]
pub struct Shutdown(
    /// 停止广播的只读接收端; 发送端消失同样结束等待.
    pub(crate) watch::Receiver<bool>,
);
impl Shutdown {
    /// 等待宿主主动停止或被丢弃.
    ///
    /// 返回不可失败的 Result, 可直接作为 Authority / Worker 的停止 future.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Shutdown` 的值; 调用后转移其所有权(Copy 类型按值复制).
    ///
    /// # 返回值
    ///
    /// 观察到停止或发送端关闭后返回 `Ok(())`; 错误类型为 Infallible, 不会返回 Err.
    pub async fn wait(mut self) -> Result<(), Infallible> {
        // 宿主消失与主动请求停止等价, 不让订阅者无限等待.
        let _ = self.0.wait_for(|stopping| *stopping).await;
        Ok(())
    }
}
