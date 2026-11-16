use anyhow::{Context, Result, ensure};
use beamlet_protocol::Pool;
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
#[serde(default, deny_unknown_fields)]
pub(crate) struct Settings {
    pub nats_url: String,
    pub color: Color,
    pub pools: BTreeSet<Pool>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            nats_url: "nats://127.0.0.1:4222".into(),
            color: Color::Auto,
            pools: BTreeSet::from([Pool::default()]),
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
        ensure!(
            !settings.pools.is_empty(),
            "Console 至少需要一个 pool: {}",
            path.display()
        );
        Ok(settings)
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
        for (source, url, color) in [
            (
                r#"(nats_url: "nats://localhost:4333", color: always)"#,
                "nats://localhost:4333",
                Color::Always,
            ),
            ("(color: never)", "nats://127.0.0.1:4222", Color::Never),
            ("(color: auto)", "nats://127.0.0.1:4222", Color::Auto),
            ("()", "nats://127.0.0.1:4222", Color::Auto),
        ] {
            fs::write(&path, source).unwrap();
            let settings = Settings::from_file(&path).unwrap();
            assert_eq!(settings.nats_url, url, "配置: {source}");
            assert_eq!(settings.color, color, "配置: {source}");
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
}
