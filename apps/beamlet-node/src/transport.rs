//! 配置到传输实例的装配, 同名引用共享消息空间.
use crate::settings;
use anyhow::{Context, Result};
use beamlet_api::AuthorityInfo;
use beamlet_authority::Transports;
use beamlet_host::Shutdown;
use beamlet_transport_local::Local;
use beamlet_transport_nats::Nats;
use std::time::Duration;
use tokio::time::timeout;

/// 已创建的命名传输实例.
#[derive(Clone)]
pub(crate) enum Transport {
    /// 进程内共享消息空间.
    Local(
        /// 克隆时共享路由和通道的句柄.
        Local,
    ),
    /// 可跨进程通信的 NATS 连接.
    Nats(
        /// 克隆时共享连接的句柄.
        Nats,
    ),
}
impl Transport {
    /// 按 config 创建实例; 连接或容量无效时返回错误.
    pub(crate) async fn connect(config: settings::Transport) -> Result<Self> {
        match config {
            settings::Transport::Local { capacity } => Ok(Self::Local(Local::new(capacity)?)),
            settings::Transport::Nats {
                url,
                connect_timeout_secs,
                subscription_capacity,
            } => {
                let deadline = Duration::from_secs(connect_timeout_secs);
                let connection = timeout(
                    deadline,
                    Nats::connect(
                        &url,
                        beamlet_transport_nats::Options {
                            connect_timeout: deadline,
                            subscription_capacity,
                        },
                    ),
                )
                .await
                .context("连接传输超时")?
                .context("连接 NATS 失败")?;
                Ok(Self::Nats(connection))
            }
        }
    }

    /// 为 info 绑定端点并加入 transports; 绑定失败时返回错误.
    pub(crate) async fn attach(
        &self,
        info: &AuthorityInfo,
        transports: &mut Transports,
    ) -> Result<()> {
        match self {
            Self::Local(local) => transports.add(local.bind_authority(info)?, local.clone()),
            Self::Nats(nats) => transports.add(nats.bind_authority(info).await?, nats.clone()),
        }
        Ok(())
    }

    /// 绑定并运行 worker, shutdown 通知后排空; 返回运行结果.
    pub(crate) async fn run_worker(
        &self,
        worker: beamlet_worker::Worker,
        shutdown: Shutdown,
    ) -> Result<()> {
        match self {
            Self::Local(local) => {
                let (inbox, presence) = local.bind_worker(worker.binding())?;
                worker.run(inbox, presence, shutdown.wait()).await?;
            }
            Self::Nats(nats) => {
                let (inbox, presence) = nats.bind_worker(worker.binding()).await?;
                worker.run(inbox, presence, shutdown.wait()).await?;
            }
        }
        Ok(())
    }

    /// 刷新此实例的发送缓冲, 返回刷新结果.
    pub(crate) async fn flush(&self) -> Result<()> {
        match self {
            Self::Local(local) => local.flush().await?,
            Self::Nats(nats) => nats.flush().await?,
        }
        Ok(())
    }
}
