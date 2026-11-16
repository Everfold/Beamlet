use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{env, fs::OpenOptions, path::Path};
use uuid::Uuid;

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Role {
    Authority,
    Worker,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Settings {
    pub node_id: Uuid,
    pub label: Option<String>,
    pub role: Role,
    pub nats_url: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            node_id: Uuid::new_v4(),
            label: None,
            role: Role::Authority,
            nats_url: "nats://127.0.0.1:4222".into(),
        }
    }
}

impl Settings {
    pub(crate) fn load() -> Result<Self> {
        let path = env::current_exe()
            .context("获取可执行文件路径失败")?
            .with_file_name("node.ron");
        Self::from_file(&path)
    }

    fn from_file(path: &Path) -> Result<Self> {
        // 串行化首次创建, 防止并发启动生成并覆盖不同的 UUID.
        let lock = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(path.with_extension("ron.lock"))
            .with_context(|| format!("打开节点配置锁失败: {}", path.display()))?;
        lock.lock()
            .with_context(|| format!("锁定节点配置失败: {}", path.display()))?;

        let settings: Self = confy::load_path(path)
            .with_context(|| format!("加载节点配置失败: {}", path.display()))?;
        ensure!(
            !settings.node_id.is_nil(),
            "node_id 不能是 nil UUID: {}",
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
        let path = directory.path().join("node.ron");
        for (role_name, role) in [("authority", Role::Authority), ("worker", Role::Worker)] {
            let node_id = Uuid::new_v4();
            let source = format!(
                r#"(node_id: "{node_id}", role: {role_name}, label: Some("华东节点"), nats_url: "nats://localhost:4333")"#
            );
            fs::write(&path, &source).unwrap();
            let settings = Settings::from_file(&path).unwrap();
            assert_eq!(settings.node_id, node_id);
            assert_eq!(settings.label.as_deref(), Some("华东节点"));
            assert_eq!(settings.role, role);
            assert_eq!(settings.nats_url, "nats://localhost:4333");
            assert_eq!(fs::read_to_string(&path).unwrap(), source);
        }
    }

    #[test]
    fn rejects_incomplete_or_invalid_config_without_rewriting_it() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("node.ron");
        let node_id = Uuid::new_v4();
        let valid = format!(
            r#"(node_id: "{node_id}", role: worker, label: None, nats_url: "nats://localhost:4333")"#
        );
        for source in [
            valid.replace(&format!("node_id: \"{node_id}\", "), ""),
            valid.replace("role: worker, ", ""),
            valid.replace("role: worker", "role: other"),
            valid.replace("nats_url", "nats_ur1"),
            valid.replace(&node_id.to_string(), "not-a-uuid"),
            valid.replace(&node_id.to_string(), &Uuid::nil().to_string()),
            valid.replace("label: None", "label: \"unsupported string\""),
            valid.replace(")", ", session_id: \"unexpected\")"),
            valid.trim_end_matches(')').to_owned(),
        ] {
            fs::write(&path, &source).unwrap();
            let error = Settings::from_file(&path).unwrap_err();
            assert!(error.to_string().contains(&path.display().to_string()));
            assert_eq!(fs::read_to_string(&path).unwrap(), source);
        }
    }

    #[test]
    fn creates_and_loads_config_on_first_start() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("node.ron");
        let settings = Settings::from_file(&path).unwrap();
        assert_eq!(settings.role, Role::Authority);
        assert_eq!(settings.nats_url, "nats://127.0.0.1:4222");
        assert!(!settings.node_id.is_nil());
        assert_eq!(settings.label, None);

        let content = fs::read_to_string(&path).unwrap();
        assert!(!content.is_empty(), "首次启动应创建配置文件");
        let reloaded = Settings::from_file(&path).unwrap();
        assert_eq!(reloaded.node_id, settings.node_id);
        assert_eq!(reloaded.role, settings.role);
        assert_eq!(reloaded.nats_url, settings.nats_url);
        assert_eq!(fs::read_to_string(&path).unwrap(), content);
    }

    #[test]
    fn independent_nodes_have_distinct_ids() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let first = Settings::from_file(&first.path().join("node.ron")).unwrap();
        let second = Settings::from_file(&second.path().join("node.ron")).unwrap();
        assert_ne!(first.node_id, second.node_id);
    }

    #[test]
    fn concurrent_first_starts_reuse_one_persisted_id() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("node.ron");
        let barrier = std::sync::Barrier::new(8);
        let ids = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..8)
                .map(|_| {
                    scope.spawn(|| {
                        barrier.wait();
                        Settings::from_file(&path).unwrap().node_id
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .collect::<Vec<_>>()
        });
        let persisted = Settings::from_file(&path).unwrap().node_id;
        assert!(ids.iter().all(|id| *id == persisted));
    }

    #[test]
    fn reports_config_creation_failure_with_path() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("missing/node.ron");
        let error = Settings::from_file(&path).unwrap_err();
        assert!(error.to_string().contains(&path.display().to_string()));
    }
}
