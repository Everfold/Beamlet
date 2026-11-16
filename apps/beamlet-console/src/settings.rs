use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{env, path::Path};

#[derive(Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Settings {
    pub nats_url: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            nats_url: "nats://127.0.0.1:4222".into(),
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
        confy::load_path(path).with_context(|| format!("加载控制台配置失败: {}", path.display()))
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
        for (source, url) in [
            (
                r#"(nats_url: "nats://localhost:4333")"#,
                "nats://localhost:4333",
            ),
            ("()", "nats://127.0.0.1:4222"),
        ] {
            fs::write(&path, source).unwrap();
            let settings = Settings::from_file(&path).unwrap();
            assert_eq!(settings.nats_url, url, "配置: {source}");
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

        let content = fs::read_to_string(&path).unwrap();
        assert!(!content.is_empty(), "首次启动应创建配置文件");
        let reloaded = Settings::from_file(&path).unwrap();
        assert_eq!(reloaded.nats_url, settings.nats_url);
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
