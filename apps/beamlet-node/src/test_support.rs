//! 真实 NATS 测试共享隔离 broker, 只清理各测试自行创建的进程.
use std::{
    net::TcpListener,
    process::{Child, Command, Stdio},
    time::Duration,
};
use tokio::time::timeout;

/// 真正启动隔离 broker, 不借用或关闭开发者的 NATS 服务.
pub(crate) struct Broker(Child);

impl Broker {
    /// 绑定空闲端口后启动服务, 连接成功才进入契约测试.
    pub(crate) async fn start() -> (Self, async_nats::Client) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let child = Command::new("nats-server")
            .args(["-a", "127.0.0.1", "-p", &port.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("需要安装 nats-server");
        let mut broker = Self(child);
        let client = timeout(Duration::from_secs(5), async {
            loop {
                assert!(broker.0.try_wait().unwrap().is_none(), "测试 NATS 提前退出");
                if let Ok(client) = async_nats::connect(format!("nats://127.0.0.1:{port}")).await {
                    break client;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("测试 NATS 未就绪");
        (broker, client)
    }
}

impl Drop for Broker {
    /// 测试断言失败时也只回收自己启动的进程.
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
