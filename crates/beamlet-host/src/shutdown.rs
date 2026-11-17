use std::convert::Infallible;
use tokio::sync::watch;

/// 只读停止通知; 所有克隆都能观察到停止, 晚开始等待也不会错过.
#[derive(Debug, Clone)]
pub struct Shutdown(pub(crate) watch::Receiver<bool>);
impl Shutdown {
    /// 等待宿主请求停止或被丢弃; 可取消后重新等待.
    ///
    /// 返回不可失败的 Result, 可直接作为 Authority / Worker 的停止 future.
    pub async fn wait(mut self) -> Result<(), Infallible> {
        // 宿主消失与主动请求停止等价, 不让订阅者无限等待.
        let _ = self.0.wait_for(|stopping| *stopping).await;
        Ok(())
    }
}
