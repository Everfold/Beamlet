//! 独立角色的应用信号与展示边界; 生命周期驱动由 beamlet 持有

use std::future::Future;

use anyhow::{Context, Result, bail};
use beamlet::lifecycle::{DEFAULT_DRAIN_TIMEOUT, Group, Shutdown, StopPolicy};
use tokio::runtime::Handle;

pub(crate) async fn standalone<F, Fut>(start: F) -> Result<()>
where
    F: FnOnce(Shutdown) -> Fut,
    Fut: Future<Output = Result<()>> + Send + 'static,
{
    let mut drivers = Group::new(Handle::current(), DEFAULT_DRAIN_TIMEOUT)?;
    drivers.start((), StopPolicy::Drain, start)?;
    let signal = tokio::select! {
        _ = drivers.wait() => Ok(()),
        signal = tokio::signal::ctrl_c() => signal.context("接收退出信号失败"),
    };
    drivers.stop();
    println!(
        "Node 进入 Draining: 停止新接纳; 等待上限={} 毫秒",
        DEFAULT_DRAIN_TIMEOUT.as_millis()
    );
    let summary = drivers.drain().await;
    if summary.timed_out {
        bail!("Node Draining 到期; 执行或交付未确认, 不生成失败 Report, 内存状态不保证恢复");
    }
    for exit in summary.exits {
        exit.result.map_err(|error| anyhow::anyhow!("{error}"))?;
    }
    signal
}
