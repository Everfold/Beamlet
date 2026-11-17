//! 完整心跳独立于任务执行, 每次连接恢复后重新声明当前状态.
use anyhow::{Context, Result};
use beamlet_api::{Presence, PresenceChannel, PresenceEvent, PresenceState};
use std::time::Duration;
use tokio::{
    sync::watch,
    time::{MissedTickBehavior, interval, timeout},
};

pub(crate) async fn announce(
    mut channel: impl PresenceChannel,
    mut snapshot: Presence,
    period: Duration,
    mut state: watch::Receiver<PresenceState>,
) -> Result<()> {
    channel.flush().await?;
    let mut ticker = interval(period);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            biased;
            changed = state.changed() => { changed.context("Worker 状态入口已关闭")?; }
            event = channel.receive() => {
                if matches!(event?, PresenceEvent::Connection(link) if link.connected) { channel.flush().await?; }
            }
            _ = ticker.tick() => {}
        }
        snapshot.state = *state.borrow_and_update();
        let connected = channel.connection().connected;
        if connected {
            snapshot.sequence = snapshot.sequence.checked_add(1).context("心跳序号已耗尽")?;
            // 断线时不积攒周期性心跳; 发送阻塞也不能拖住退出.
            match timeout(Duration::from_secs(1), channel.publish(&snapshot)).await {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    tracing::warn!(event = "heartbeat.failed", %error, "心跳发送失败")
                }
                Err(_) => tracing::warn!(event = "heartbeat.failed", "心跳发送超时"),
            }
        }
        if snapshot.state == PresenceState::Offline {
            if connected {
                timeout(Duration::from_secs(1), channel.flush()).await??;
            }
            return Ok(());
        }
    }
}
