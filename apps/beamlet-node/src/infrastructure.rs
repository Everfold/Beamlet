//! 基础设施连接及其存活状态, 可由同一 Node 内的多个角色共享.
use anyhow::{Context, Result};
use tokio::sync::watch;
use tracing::info;

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
    pub(crate) async fn connect(url: &str) -> Result<Self> {
        let (connection, status) = watch::channel(Connection {
            epoch: 0,
            connected: false,
        });

        let events = connection.clone();

        let client = async_nats::ConnectOptions::new()
            .subscription_capacity(crate::delivery::MAX_IN_FLIGHT)
            .event_callback(move |event| {
                let events = events.clone();
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
