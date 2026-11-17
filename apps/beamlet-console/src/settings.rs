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
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn loads_existing_config_without_overwriting_it() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("console.ron");
        for color in [Color::Always, Color::Never, Color::Auto] {
            let config = Settings {
                nats_url: "nats://localhost:4333".into(),
                color,
                request_timeout_secs: 30,
                discovery_window_ms: 250,
                max_in_flight_requests: 7,
                connect_timeout_secs: 20,
                subscription_capacity: 32,
                ..Settings::default()
            };
            confy::store_path(&path, &config).unwrap();
            let source = fs::read_to_string(&path).unwrap();
            let settings = Settings::from_file(&path).unwrap();
            assert_eq!(settings.nats_url, config.nats_url);
            assert_eq!(settings.color, color);
            assert_eq!(settings.request_timeout_secs, 30);
            assert_eq!(settings.discovery_window_ms, 250);
            assert_eq!(settings.max_in_flight_requests, 7);
            assert_eq!(settings.connect_timeout_secs, 20);
            assert_eq!(settings.subscription_capacity, 32);
            assert_eq!(fs::read_to_string(&path).unwrap(), source);
        }
    }

    #[test]
    fn rejects_unknown_fields_and_invalid_ron() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("console.ron");
        for source in [
            r#"(nats_ur1: "nats://localhost:4333")"#,
            r#"(nats_url: "nats://localhost:4333""#,
            "(color: invalid)",
        ] {
            fs::write(&path, source).unwrap();
            let error = Settings::from_file(&path).unwrap_err();
            assert!(error.to_string().contains(&path.display().to_string()));
            assert_eq!(fs::read_to_string(&path).unwrap(), source);
        }
    }

    #[test]
    fn creates_and_loads_config_on_first_start() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("console.ron");
        let settings = Settings::from_file(&path).unwrap();
        assert_eq!(settings.nats_url, "nats://127.0.0.1:4222");
        assert_eq!(settings.color, Color::Auto);

        let content = fs::read_to_string(&path).unwrap();
        assert!(!content.is_empty(), "首次启动应创建配置文件");
        let reloaded = Settings::from_file(&path).unwrap();
        assert_eq!(reloaded.nats_url, settings.nats_url);
        assert_eq!(reloaded.color, settings.color);
        assert_eq!(fs::read_to_string(&path).unwrap(), content);
    }

    #[test]
    fn reports_config_creation_failure_with_path() {
        let directory = tempfile::tempdir().unwrap();
        let parent = directory.path().join("not-a-directory");
        fs::write(&parent, "occupied").unwrap();
        let path = parent.join("console.ron");
        let error = Settings::from_file(&path).unwrap_err();
        assert!(error.to_string().contains(&path.display().to_string()));
    }
    #[test]
    fn deadlines_and_capacity_reject_missing_zero_negative_and_overflow_without_rewriting() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("console.ron");
        confy::store_path(&path, Settings::default()).unwrap();
        let source = fs::read_to_string(&path).unwrap();
        for field in [
            "request_timeout_secs: 10",
            "connect_timeout_secs: 10",
            "subscription_capacity: 65536",
            "discovery_window_ms: 1000",
            "max_in_flight_requests: 16",
        ] {
            let name = field.split(':').next().unwrap();
            for replacement in [
                String::new(),
                format!("{name}: 0"),
                format!("{name}: -1"),
                format!("{name}: 18446744073709551616"),
            ] {
                let invalid = source.replace(
                    &format!("{field},"),
                    &if replacement.is_empty() {
                        replacement
                    } else {
                        format!("{replacement},")
                    },
                );
                assert_ne!(invalid, source);
                fs::write(&path, &invalid).unwrap();
                assert!(
                    Settings::from_file(&path).is_err(),
                    "accepted {name}: {invalid}"
                );
                assert_eq!(fs::read_to_string(&path).unwrap(), invalid);
            }
        }
    }
    #[test]
    fn discovery_window_must_leave_time_for_network_setup() {
        let mut settings = Settings {
            request_timeout_secs: 2,
            discovery_window_ms: 1500,
            max_in_flight_requests: 3,
            ..Default::default()
        };
        settings.validate().unwrap();
        for window in [2000, 2001, u64::MAX] {
            settings.discovery_window_ms = window;
            assert!(settings.validate().is_err());
        }
    }
}
