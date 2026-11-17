//! 完整心跳独立于任务执行, 每次连接恢复后重新声明当前状态.
use crate::{Options, RunError};
use beamlet_api::{Presence, PresenceChannel, PresenceEvent, PresenceState};
use tokio::{
    sync::watch,
    time::{MissedTickBehavior, interval, timeout},
};

/// 响应周期、探测、连接和状态变化，发布递增序号的完整公告。
///
/// # 参数
///
/// - `channel`：Worker 的探测、连接变化和完整公告通道。
/// - `snapshot`：Worker 初始完整公告；驱动循环更新其序号和状态。
/// - `options`：当前模块的运行参数；期限使用 Duration，容量/并发由对应字段约束。
/// - `state`：执行循环与公告循环之间共享的 Worker 状态通知。
///
/// # 返回值
///
/// Offline 收尾完成返回 `Ok(())`；控制入口、序号耗尽或必要刷新失败返回 RunError；单次公告发送失败仅记录日志。
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

/// 在公告期限内刷新 Worker 的传输缓冲。
///
/// # 参数
///
/// - `channel`：Worker 的探测、连接变化和完整公告通道。
/// - `options`：当前模块的运行参数；期限使用 Duration，容量/并发由对应字段约束。
///
/// # 返回值
///
/// 成功返回 `Ok(())`；超时或刷新失败返回 RunError。
async fn flush(channel: &impl PresenceChannel, options: Options) -> Result<(), RunError> {
    timeout(options.announcement_timeout, channel.flush())
        .await
        .map_err(|_| RunError::AnnouncementTimeout)??;
    Ok(())
}
