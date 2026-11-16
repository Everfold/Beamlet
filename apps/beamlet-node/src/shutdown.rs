//! 一个 Node 的退出请求广播给所有角色; 订阅者晚轮询也不会错过退出.
use std::io;
use tokio::sync::watch;

#[derive(Clone)]
pub(crate) struct Shutdown(watch::Receiver<bool>);

pub(crate) fn channel() -> (watch::Sender<bool>, Shutdown) {
    let (send, recv) = watch::channel(false);
    (send, Shutdown(recv))
}
impl Shutdown {
    pub(crate) async fn wait(mut self) -> io::Result<()> {
        self.0
            .wait_for(|stopping| *stopping)
            .await
            .map_err(|_| io::Error::other("Node 退出信号入口已关闭"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn all_roles_observe_shutdown_including_late_pollers() {
        let (stop, shutdown) = channel();
        let early = tokio::spawn(shutdown.clone().wait());
        stop.send_replace(true);
        early.await.unwrap().unwrap();
        shutdown.wait().await.unwrap();
    }
}
