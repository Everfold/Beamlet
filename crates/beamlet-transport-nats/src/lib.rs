//! NATS 上的定向收发与连接观测, 不保存角色策略或 Operation 状态.
mod authority;
mod client;
mod delivery;
pub mod subject;
mod worker;

pub use authority::AuthorityEndpoint;
pub use client::Discovery;
pub use worker::{PresenceEndpoint, WorkerEndpoint};

use beamlet_api::{AuthorityInfo, Connection, ConnectionView, TransportError, WorkerBinding};
use std::time::Duration;
use tokio::{sync::watch, time::timeout};
use tracing::{Instrument, info};

/// 每个连接实例的配置, 可由任意配置来源构造.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// 连接完成的最长等待时间.
    pub connect_timeout: Duration,
    /// 每个订阅的接收缓冲容量.
    pub subscription_capacity: usize,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            subscription_capacity: 16,
        }
    }
}

/// 可被多个角色共享的连接; clone 共享连接, 再次 connect 创建独立连接.
#[derive(Debug, Clone)]
pub struct Nats {
    client: async_nats::Client,
    status: watch::Receiver<Connection>,
}
impl Nats {
    /// 建立连接并启用连接世代观测, 需要调用方的 Tokio 执行器.
    ///
    /// # Errors
    /// 参数无效、连接失败或超过期限时返回错误.
    pub async fn connect(url: &str, options: Options) -> Result<Self, TransportError> {
        if options.connect_timeout.is_zero()
            || std::time::Instant::now()
                .checked_add(options.connect_timeout)
                .is_none()
            || options.subscription_capacity == 0
            || options.subscription_capacity > tokio::sync::Semaphore::MAX_PERMITS
        {
            return Err(TransportError::new(
                "invalid connection timeout or subscription capacity",
            ));
        }
        let (connection, status) = watch::channel(Connection {
            epoch: 0,
            connected: false,
        });
        let events = connection.clone();
        let span = tracing::Span::current();
        let connecting = async_nats::ConnectOptions::new()
            .subscription_capacity(options.subscription_capacity)
            .connection_timeout(options.connect_timeout)
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
                        events.send_if_modified(|state| {
                            if state.epoch != 0 && state.connected == connected {
                                return false;
                            }
                            state.epoch = state
                                .epoch
                                .checked_add(1)
                                .expect("connection epoch exhausted");
                            state.connected = connected;
                            true
                        });
                        info!(
                            event = "transport.connection",
                            connected, "NATS 连接状态变化"
                        );
                    }
                }
                .instrument(span)
            })
            .connect(url);
        let client = timeout(options.connect_timeout, connecting)
            .await
            .map_err(TransportError::new)?
            .map_err(TransportError::new)?;
        connection.send_if_modified(|state| {
            if state.epoch == 0 {
                *state = Connection {
                    epoch: 1,
                    connected: true,
                };
                true
            } else {
                false
            }
        });
        Ok(Self { client, status })
    }
    /// 获取可共享的只读观测视图.
    pub fn connection(&self) -> ConnectionView {
        let status = self.status.clone();
        ConnectionView::new(move || *status.borrow())
    }
    /// 绑定 Authority 控制接收端, 同一连接可绑定多个实例.
    ///
    /// # Errors
    /// 身份或 pools 无效, 订阅或刷新失败时返回错误.
    pub async fn bind_authority(
        &self,
        info: &AuthorityInfo,
    ) -> Result<AuthorityEndpoint, TransportError> {
        AuthorityEndpoint::bind(self.clone(), info).await
    }
    /// 绑定调用与公告通道, 绑定完成后角色才可以公告 ready.
    ///
    /// # Errors
    /// 身份或能力无效, 订阅或刷新失败时返回错误.
    pub async fn bind_worker(
        &self,
        binding: WorkerBinding,
    ) -> Result<(WorkerEndpoint, PresenceEndpoint), TransportError> {
        worker::bind(self.clone(), binding).await
    }
    /// 等待缓冲发送完毕, 不保证远端业务接受.
    ///
    /// # Errors
    /// NATS 刷新失败时返回错误.
    pub async fn flush(&self) -> Result<(), TransportError> {
        self.client.flush().await.map_err(TransportError::new)
    }
}
