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
    /// 构造 NATS 默认连接期限和订阅容量。
    ///
    /// # 参数
    ///
    /// 无。
    ///
    /// # 返回值
    ///
    /// 连接期限 10 秒、订阅容量 16 的 Options。
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
    /// 校验 NATS 选项并在连接期限内建立客户端连接。
    ///
    /// # 参数
    ///
    /// - `url`：NATS 服务地址，由调用方从任意配置来源提供。
    /// - `options`：当前模块的运行参数；期限使用 Duration，容量/并发由对应字段约束。
    ///
    /// # 返回值
    ///
    /// 成功返回 Nats 句柄；参数非法、连接失败或超时返回 TransportError。
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
    /// 取得能够持续读取当前连接状态的视图。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Nats` 的共享引用。
    ///
    /// # 返回值
    ///
    /// 连接视图句柄；后续查询反映连接当前世代和状态。
    pub fn connection(&self) -> ConnectionView {
        let status = self.status.clone();
        ConnectionView::new(move || *status.borrow())
    }
    /// 订阅 Authority 各个 pool 的控制主题并刷新订阅。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Nats` 的共享引用。
    /// - `info`：Authority 的角色/会话身份、标签及其加入的 pools。
    ///
    /// # 返回值
    ///
    /// 成功返回 AuthorityEndpoint；绑定非法、订阅或刷新失败返回 TransportError。
    ///
    /// # Errors
    /// 身份或 pools 无效, 订阅或刷新失败时返回错误.
    pub async fn bind_authority(
        &self,
        info: &AuthorityInfo,
    ) -> Result<AuthorityEndpoint, TransportError> {
        AuthorityEndpoint::bind(self.clone(), info).await
    }
    /// 订阅指定 Worker 会话的调用和探测主题。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Nats` 的共享引用。
    /// - `binding`：Worker 的 pool、角色/会话身份和启用能力集合。
    ///
    /// # 返回值
    ///
    /// 成功返回 `(WorkerEndpoint, PresenceEndpoint)`；绑定、订阅或刷新失败返回 TransportError。
    ///
    /// # Errors
    /// 身份或能力无效, 订阅或刷新失败时返回错误.
    pub async fn bind_worker(
        &self,
        binding: WorkerBinding,
    ) -> Result<(WorkerEndpoint, PresenceEndpoint), TransportError> {
        worker::bind(self.clone(), binding).await
    }
    /// 等待传输缓冲完成刷新。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Nats` 的共享引用。
    ///
    /// # 返回值
    ///
    /// 刷新完成返回 `Ok(())`；失败返回 TransportError，成功不代表远端已接受业务。
    ///
    /// # Errors
    /// NATS 刷新失败时返回错误.
    pub async fn flush(&self) -> Result<(), TransportError> {
        self.client.flush().await.map_err(TransportError::new)
    }
}
