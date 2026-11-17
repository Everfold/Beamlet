//! 完整心跳独立于任务执行, 每次连接恢复后重新声明当前状态.
use crate::{Options, RunError};
use beamlet_api::{Presence, PresenceChannel, PresenceEvent, PresenceState};
use tokio::{
    sync::watch,
    time::{MissedTickBehavior, interval, timeout},
};

pub(crate) async fn announce(
    mut channel: impl PresenceChannel,
    mut snapshot: Presence,
    options: Options,
    mut state: watch::Receiver<PresenceState>,
) -> Result<(), RunError> {
    flush(&channel, options).await?;
    let mut ticker = interval(options.heartbeat_interval);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            biased;
            changed = state.changed() => { changed.map_err(|_| RunError::AnnouncementStopped)?; }
            event = channel.receive() => {
                if matches!(event?, PresenceEvent::Connection(link) if link.connected) { flush(&channel, options).await?; }
            }
            _ = ticker.tick() => {}
        }
        snapshot.state = *state.borrow_and_update();
        let connected = channel.connection().connected;
        if connected {
            snapshot.sequence = snapshot
                .sequence
                .checked_add(1)
                .ok_or(RunError::PresenceSequenceExhausted)?;
            // 断线时不积攒周期性心跳; 发送阻塞也不能拖住退出.
            match timeout(options.announcement_timeout, channel.publish(&snapshot)).await {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    tracing::warn!(event = "heartbeat.failed", %error, "心跳发送失败")
                }
                Err(_) => tracing::warn!(event = "heartbeat.failed", "心跳发送超时"),
            }
        }
        if snapshot.state == PresenceState::Offline {
            if connected {
                flush(&channel, options).await?;
            }
            return Ok(());
        }
    }
}

async fn flush(channel: &impl PresenceChannel, options: Options) -> Result<(), RunError> {
    timeout(options.announcement_timeout, channel.flush())
        .await
        .map_err(|_| RunError::AnnouncementTimeout)??;
    Ok(())
}
