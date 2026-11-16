use anyhow::{Context, Result};
use config::{Config, File, FileFormat};
use serde::Deserialize;
use std::{
    env,
    fs::OpenOptions,
    io::{ErrorKind, Write},
    path::Path,
};

/// 首次启动配置.
const DEFAULT_CONFIG: &str = r#"(
    nats_url: "nats://127.0.0.1:4222",
)
"#;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Settings {
    pub nats_url: String,
}

impl Settings {
    pub(crate) fn load() -> Result<Self> {
        let path = env::current_exe()
            .context("获取可执行文件路径失败")?
            .with_file_name("console.ron");
        Self::from_file(&path)
    }

    fn from_file(path: &Path) -> Result<Self> {
        match OpenOptions::new().write(true).create_new(true).open(path) {
            Ok(mut file) => file
                .write_all(DEFAULT_CONFIG.as_bytes())
                .with_context(|| format!("写入默认控制台配置失败: {}", path.display()))?,
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("创建控制台配置失败: {}", path.display()));
            }
        }

        Config::builder()
            .set_default("nats_url", "nats://127.0.0.1:4222")?
            .add_source(File::from(path).format(FileFormat::Ron).required(true))
            .build()
            .and_then(Config::try_deserialize)
            .with_context(|| format!("加载控制台配置失败: {}", path.display()))
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
            ("{}", "nats://127.0.0.1:4222"),
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
        let path = directory.path().join("missing/console.ron");
        let error = Settings::from_file(&path).unwrap_err();
        assert!(error.to_string().contains(&path.display().to_string()));
    }
}
