//! 基础设施连接及其存活状态, 可由同一 Node 内的多个角色共享.
use anyhow::{Context, Result};
use std::time::Duration;
use tokio::sync::watch;
use tracing::{Instrument, info};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Connection {
    pub epoch: u64,
    pub connected: bool,
}

#[derive(Clone)]
pub(crate) struct Nats {
    pub client: async_nats::Client,
    pub status: watch::Receiver<Connection>,
}

impl Nats {
    pub(crate) async fn connect(
        url: &str,
        connect_timeout: Duration,
        subscription_capacity: usize,
    ) -> Result<Self> {
        let (connection, status) = watch::channel(Connection {
            epoch: 0,
            connected: false,
        });

        let events = connection.clone();
        let span = tracing::Span::current();

        let client = async_nats::ConnectOptions::new()
            .subscription_capacity(subscription_capacity)
            .connection_timeout(connect_timeout)
            .event_callback(move |event| {
                let events = events.clone();
                let span = span.clone();
                async move {
                    let connected = match event {
                        async_nats::Event::Connected => Some(true),
                        async_nats::Event::Disconnected | async_nats::Event::Closed => Some(false),
                        _ => None,
                    };
                    if let Some(connected) = connected {
                        events.send_modify(|state| {
                            state.epoch += 1;
                            state.connected = connected;
                        });
                        info!(
                            event = "transport.connection",
                            connected, "NATS 连接状态变化"
                        );
                    }
                }
                .instrument(span)
            })
            .connect(url)
            .await
            .context("连接 NATS 失败")?;

        // connect 完成先建立初始状态; 后续事件始终推进连接世代.
        connection.send_if_modified(|state| {
            if state.epoch == 0 {
                state.epoch = 1;
                state.connected = true;
                true
            } else {
                false
            }
        });

        Ok(Self { client, status })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn configured_connection_timeout_covers_a_server_that_never_sends_info() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let connection = Nats::connect(&address, Duration::from_millis(30), 2);
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
}
