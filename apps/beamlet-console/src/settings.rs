use anyhow::{Context, Result, ensure};
use beamlet_api::Pool;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::{env, io::IsTerminal, path::Path};

#[derive(Debug, Default, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Color {
    #[default]
    Auto,
    Always,
    Never,
}

impl Color {
    pub(crate) fn enabled(self) -> bool {
        match self {
            Self::Auto => {
                std::io::stderr().is_terminal()
                    && env::var_os("NO_COLOR").is_none_or(|value| value.is_empty())
            }
            Self::Always => true,
            Self::Never => false,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Settings {
    pub nats_url: String,
    pub color: Color,
    pub pools: BTreeSet<Pool>,
    pub request_timeout_secs: u64,
    pub discovery_window_ms: u64,
    pub max_in_flight_requests: usize,
    pub connect_timeout_secs: u64,
    pub subscription_capacity: usize,
}

impl From<&Settings> for beamlet_client::Options {
    fn from(settings: &Settings) -> Self {
        Self {
            request_timeout: std::time::Duration::from_secs(settings.request_timeout_secs),
            discovery_window: std::time::Duration::from_millis(settings.discovery_window_ms),
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            nats_url: "nats://127.0.0.1:4222".into(),
            color: Color::Auto,
            pools: BTreeSet::from([Pool::default()]),
            request_timeout_secs: 10,
            discovery_window_ms: 1000,
            max_in_flight_requests: 16,
            connect_timeout_secs: 10,
            subscription_capacity: 65536,
        }
    }
}

impl Settings {
    pub(crate) fn load() -> Result<Self> {
        let path = env::current_exe()
            .context("获取可执行文件路径失败")?
            .with_file_name("console.ron");
        Self::from_file(&path)
    }

    fn from_file(path: &Path) -> Result<Self> {
        let settings: Self = confy::load_path(path)
            .with_context(|| format!("加载控制台配置失败: {}", path.display()))?;
        settings
            .validate()
            .with_context(|| format!("控制台配置无效: {}", path.display()))?;
        Ok(settings)
    }
    pub(crate) fn validate(&self) -> Result<()> {
        ensure!(!self.pools.is_empty(), "Console 至少需要一个 pool");
        ensure!(!self.nats_url.trim().is_empty(), "nats_url 不能为空");
        for (name, seconds) in [
            ("request_timeout_secs", self.request_timeout_secs),
            ("connect_timeout_secs", self.connect_timeout_secs),
        ] {
            ensure!(
                seconds > 0
                    && std::time::Instant::now()
                        .checked_add(std::time::Duration::from_secs(seconds))
                        .is_some(),
                "{name} 必须大于 0 且在系统时钟范围内"
            );
        }
        ensure!(
            self.discovery_window_ms > 0
                && std::time::Duration::from_millis(self.discovery_window_ms)
                    < std::time::Duration::from_secs(self.request_timeout_secs),
            "discovery_window_ms 必须大于 0 且小于 request_timeout_secs 指定的期限"
        );
        ensure!(
            self.max_in_flight_requests > 0,
            "max_in_flight_requests 必须大于 0"
        );
        ensure!(
            self.subscription_capacity > 0
                && self.subscription_capacity <= tokio::sync::Semaphore::MAX_PERMITS,
            "subscription_capacity 必须大于 0 且不超过通道容量范围"
        );
        Ok(())
    }
}

#[cfg(test)]
#[path = "../tests/unit/settings.rs"]
mod tests;
