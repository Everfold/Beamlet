//! 完整心跳独立于任务执行, 每次连接恢复后重新声明当前状态.
use crate::infrastructure::Connection;
use anyhow::{Context, Result};
use beamlet_api::{Presence, PresenceState};
use beamlet_protocol::{encode, subject};
use futures::StreamExt;
use std::time::Duration;
use tokio::{
    sync::watch,
    time::{MissedTickBehavior, interval, timeout},
};

pub(crate) async fn announce(
    client: async_nats::Client,
    mut snapshot: Presence,
    period: Duration,
    mut state: watch::Receiver<PresenceState>,
    mut connection: watch::Receiver<Connection>,
) -> Result<()> {
    let mut probes = client.subscribe(subject::probe(&snapshot.pool)).await?;
    client.flush().await?;
    let mut ticker = interval(period);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            biased;
            changed = state.changed() => { changed.context("Worker 状态入口已关闭")?; }
            changed = connection.changed() => {
                changed.context("连接状态入口已关闭")?;
                if connection.borrow().connected { client.flush().await?; }
            }
            probe = probes.next() => { probe.context("Worker 探测订阅已关闭")?; }
            _ = ticker.tick() => {}
        }
        snapshot.state = *state.borrow_and_update();
        let connected = connection.borrow_and_update().connected;
        if connected {
            snapshot.sequence = snapshot.sequence.checked_add(1).context("心跳序号已耗尽")?;
            let bytes = encode(&snapshot)?;
            // 断线时不积攒周期性心跳; 发送阻塞也不能拖住退出.
            match timeout(
                Duration::from_secs(1),
                client.publish(subject::presence(&snapshot.pool), bytes.into()),
            )
            .await
            {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    tracing::warn!(event = "heartbeat.failed", %error, "心跳发送失败")
                }
                Err(_) => tracing::warn!(event = "heartbeat.failed", "心跳发送超时"),
            }
        }
        if snapshot.state == PresenceState::Offline {
            if connected {
                timeout(Duration::from_secs(1), client.flush()).await??;
            }
            return Ok(());
        }
    }
}
