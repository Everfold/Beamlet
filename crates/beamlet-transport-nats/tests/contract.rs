/// 本后端与其他适配器共同执行的交付和控制契约测试.
#[path = "../../../tests/support/transport_contract.rs"]
mod contract;
use beamlet_transport_nats::{Nats, Options};
use std::{
    net::TcpListener,
    process::{Child, Command, Stdio},
    time::Duration,
};

/// 测试子进程的清理 guard; 离开作用域时尽力终止并回收服务进程.
struct Server(
    /// 由当前测试启动并负责回收的子进程句柄.
    Child,
);
impl Drop for Server {
    /// 结束测试服务子进程并回收其退出状态.
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
/// 启动临时 NATS 服务并连接待验收的适配器.
async fn transport() -> (Server, Nats) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let server = Server(
        Command::new("nats-server")
            .args(["-a", "127.0.0.1", "-p", &port.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("nats-server must be on PATH"),
    );
    let url = format!("nats://127.0.0.1:{port}");
    let connected = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match Nats::connect(&url, Options::default()).await {
                Ok(transport) => break transport,
                Err(_) => tokio::time::sleep(Duration::from_millis(20)).await,
            }
        }
    })
    .await
    .unwrap();
    (server, connected)
}
/// 用真实 NATS 服务验证共享的定向交付契约.
#[tokio::test]
#[ignore = "requires nats-server"]
async fn delivery_contract() {
    let (_server, transport) = transport().await;
    let (inbox, _presence) = transport.bind_worker(contract::binding()).await.unwrap();
    contract::delivery(transport, inbox).await;
}
/// 用真实 NATS 服务验证共享的发现, 请求和公告控制契约.
#[tokio::test]
#[ignore = "requires nats-server"]
async fn control_contract() {
    let (_server, transport) = transport().await;
    let endpoint = transport
        .bind_authority(&contract::authority())
        .await
        .unwrap();
    let (_worker, presence) = transport.bind_worker(contract::binding()).await.unwrap();
    contract::control(transport, endpoint, presence).await;
}

/// 验证连接期限覆盖 TCP 已连接但服务端不发送 INFO 的场景.
#[tokio::test]
async fn configured_connection_timeout_covers_a_server_that_never_sends_info() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let connection = Nats::connect(
        &address,
        Options {
            connect_timeout: Duration::from_millis(30),
            subscription_capacity: 2,
        },
    );
    tokio::pin!(connection);
    let socket = tokio::select! {
        accepted = listener.accept() => accepted.unwrap().0,
        _ = &mut connection => panic!("connection ended before accepting TCP"),
    };
    let result = tokio::time::timeout(Duration::from_secs(1), connection)
        .await
        .expect("connection timeout ignored");
    assert!(result.is_err());
    drop(socket);
}
