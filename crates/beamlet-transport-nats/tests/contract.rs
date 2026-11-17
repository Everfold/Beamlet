#[path = "../../../tests/support/transport_contract.rs"]
mod contract;
use beamlet_transport_nats::{Nats, Options};
use std::{
    net::TcpListener,
    process::{Child, Command, Stdio},
    time::Duration,
};

struct Server(Child);
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
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
#[tokio::test]
#[ignore = "requires nats-server"]
async fn delivery_contract() {
    let (_server, transport) = transport().await;
    let (inbox, _presence) = transport.bind_worker(contract::binding()).await.unwrap();
    contract::delivery(transport, inbox).await;
}
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
