use anyhow::{Context, Result, ensure};
use beamlet_api::Pool;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    fs::OpenOptions,
    io::IsTerminal,
    path::Path,
};
use uuid::Uuid;

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

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Infrastructure {
    Nats {
        url: String,
        connect_timeout_secs: u64,
        subscription_capacity: usize,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Authority {
    pub pools: BTreeSet<Pool>,
    pub worker_timeout_secs: u64,
    pub report_timeout_secs: u64,
    /// 本地和远端业务入口各自的队列容量.
    pub input_capacity: usize,
    /// 等待发送的结果/拒绝回复上限, 必须大于 0.
    pub max_concurrent_replies: usize,
    /// 同时交付并等待报告的任务上限; 0 不限制.
    pub max_concurrent_tasks: usize,
}
impl Default for Authority {
    fn default() -> Self {
        Self {
            pools: BTreeSet::from([Pool::default()]),
            worker_timeout_secs: 15,
            report_timeout_secs: 3,
            input_capacity: 16,
            max_concurrent_replies: 16,
            max_concurrent_tasks: 16,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Worker {
    pub pool: Pool,
    pub capabilities: BTreeSet<String>,
    pub heartbeat_interval_secs: u64,
    /// 同时执行并发送报告的任务上限; 0 不限制.
    pub max_concurrent_tasks: usize,
}
impl Default for Worker {
    fn default() -> Self {
        Self {
            pool: Pool::default(),
            capabilities: BTreeSet::from([crate::text_reverse::CAPABILITY.into()]),
            heartbeat_interval_secs: 5,
            max_concurrent_tasks: 1,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Role {
    Authority(Authority),
    Worker(Worker),
}
impl Role {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Authority(_) => "authority",
            Self::Worker(_) => "worker",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RoleSettings {
    pub role_id: Uuid,
    pub label: Option<String>,
    pub infrastructure: String,
    pub kind: Role,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Settings {
    pub node_id: Uuid,
    pub label: Option<String>,
    pub color: Color,
    pub shutdown_timeout_secs: u64,
    pub infrastructures: BTreeMap<String, Infrastructure>,
    pub roles: Vec<RoleSettings>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            node_id: Uuid::new_v4(),
            label: None,
            color: Color::Auto,
            shutdown_timeout_secs: 6,
            infrastructures: BTreeMap::from([(
                "local".into(),
                Infrastructure::Nats {
                    url: "nats://127.0.0.1:4222".into(),
                    connect_timeout_secs: 10,
                    subscription_capacity: 16,
                },
            )]),
            roles: vec![
                RoleSettings {
                    role_id: Uuid::new_v4(),
                    label: Some("authority".into()),
                    infrastructure: "local".into(),
                    kind: Role::Authority(Authority::default()),
                },
                RoleSettings {
                    role_id: Uuid::new_v4(),
                    label: Some("worker".into()),
                    infrastructure: "local".into(),
                    kind: Role::Worker(Worker::default()),
                },
            ],
        }
    }
}
impl Settings {
    pub(crate) fn load(file_name: &Path) -> Result<Self> {
        ensure!(
            file_name.file_name() == Some(file_name.as_os_str()),
            "配置文件名称必须是文件名, 不能包含目录: {}",
            file_name.display()
        );
        let path = env::current_exe()
            .context("获取可执行文件路径失败")?
            .with_file_name(file_name);
        Self::from_file(&path)
    }
    fn from_file(path: &Path) -> Result<Self> {
        // 串行化首次创建, 防止并发启动覆盖不同的 Node / 角色身份.
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
        settings
            .validate()
            .with_context(|| format!("节点配置无效: {}", path.display()))?;
        Ok(settings)
    }
    pub(crate) fn validate(&self) -> Result<()> {
        validate_timeout(self.shutdown_timeout_secs, "shutdown_timeout_secs")?;
        ensure!(!self.node_id.is_nil(), "node_id 不能是 nil UUID");
        ensure!(!self.infrastructures.is_empty(), "至少需要一个基础设施实例");
        for (name, infrastructure) in &self.infrastructures {
            ensure!(!name.trim().is_empty(), "基础设施名称不能为空");
            match infrastructure {
                Infrastructure::Nats {
                    url,
                    connect_timeout_secs,
                    subscription_capacity,
                } => {
                    ensure!(
                        !url.trim().is_empty(),
                        "基础设施 {name} 的 NATS 地址不能为空"
                    );
                    validate_timeout(*connect_timeout_secs, "connect_timeout_secs")?;
                    validate_capacity(*subscription_capacity, "subscription_capacity")?;
                }
            }
        }
        // 允许只配置基础设施的 Node, 为后续基础设施类型保留相同装配方式.
        let mut ids = BTreeSet::new();
        for role in &self.roles {
            ensure!(!role.role_id.is_nil(), "role_id 不能是 nil UUID");
            ensure!(ids.insert(role.role_id), "重复的 role_id: {}", role.role_id);
            ensure!(
                self.infrastructures.contains_key(&role.infrastructure),
                "角色 {} 引用了不存在的基础设施: {}",
                role.role_id,
                role.infrastructure
            );
            match &role.kind {
                Role::Authority(config) => {
                    validate_timeout(config.report_timeout_secs, "report_timeout_secs")?;
                    validate_capacity(config.input_capacity, "input_capacity")?;
                    validate_capacity(config.max_concurrent_replies, "max_concurrent_replies")?;
                    ensure!(
                        !config.pools.is_empty(),
                        "Authority {} 至少需要一个 pool",
                        role.role_id
                    );
                    ensure!(
                        config.worker_timeout_secs > 0,
                        "worker_timeout_secs 必须大于 0"
                    );
                }
                Role::Worker(config) => {
                    ensure!(
                        config.heartbeat_interval_secs > 0,
                        "heartbeat_interval_secs 必须大于 0"
                    );
                    crate::capabilities::validate(&config.capabilities)
                        .with_context(|| format!("Worker {} 能力配置无效", role.role_id))?;
                }
            }
        }
        Ok(())
    }
}

fn validate_timeout(seconds: u64, name: &str) -> Result<()> {
    ensure!(
        seconds > 0
            && std::time::Instant::now()
                .checked_add(std::time::Duration::from_secs(seconds))
                .is_some(),
        "{name} 必须大于 0 且在系统时钟范围内"
    );
    Ok(())
}
fn validate_capacity(capacity: usize, name: &str) -> Result<()> {
    ensure!(
        capacity > 0 && capacity <= tokio::sync::Semaphore::MAX_PERMITS,
        "{name} 必须大于 0 且不超过通道容量范围"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn first_start_creates_distinct_persistent_ids_and_reload_does_not_write() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("node.ron");
        let first = Settings::from_file(&path).unwrap();
        assert_eq!(first.color, Color::Auto);
        assert_eq!(first.roles.len(), 2);
        let ids: BTreeSet<_> = std::iter::once(first.node_id)
            .chain(first.roles.iter().map(|role| role.role_id))
            .collect();
        assert_eq!(ids.len(), 3);
        assert!(!ids.contains(&Uuid::nil()));
        let content = fs::read_to_string(&path).unwrap();
        assert!(!content.contains("session_id"));
        let again = Settings::from_file(&path).unwrap();
        assert_eq!(again.node_id, first.node_id);
        assert_eq!(
            again
                .roles
                .iter()
                .map(|role| role.role_id)
                .collect::<Vec<_>>(),
            first
                .roles
                .iter()
                .map(|role| role.role_id)
                .collect::<Vec<_>>()
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), content);
    }

    #[test]
    fn independently_configures_repeated_role_and_infrastructure_types() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("node.ron");
        let mut settings = Settings::default();
        settings.infrastructures.insert(
            "secondary".into(),
            Infrastructure::Nats {
                url: "nats://localhost:4333".into(),
                connect_timeout_secs: 20,
                subscription_capacity: 32,
            },
        );
        let mut extra = settings.roles[1].clone();
        extra.role_id = Uuid::new_v4();
        extra.infrastructure = "secondary".into();
        extra.kind = Role::Worker(Worker {
            pool: "other".parse().unwrap(),
            capabilities: BTreeSet::from([crate::capabilities::UPPERCASE.into()]),
            heartbeat_interval_secs: 1,
            max_concurrent_tasks: 4,
        });
        settings.roles.push(extra);
        confy::store_path(&path, &settings).unwrap();
        let source = fs::read_to_string(&path).unwrap();
        let loaded = Settings::from_file(&path).unwrap();
        assert_eq!(loaded.roles.len(), 3);
        assert_eq!(loaded.infrastructures.len(), 2);
        let Role::Worker(worker) = &loaded.roles[2].kind else {
            panic!("expected worker")
        };
        assert_eq!(worker.heartbeat_interval_secs, 1);
        assert_eq!(worker.max_concurrent_tasks, 4);
        assert_eq!(worker.pool.to_string(), "other");
        assert_eq!(fs::read_to_string(&path).unwrap(), source);
    }

    #[test]
    fn invalid_or_missing_role_ids_are_not_generated_or_written_back() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("node.ron");
        let mut settings = Settings::default();
        settings.roles[1].role_id = settings.roles[0].role_id;
        confy::store_path(&path, &settings).unwrap();
        let duplicate = fs::read_to_string(&path).unwrap();
        let distinct = duplicate.replacen(
            &settings.roles[0].role_id.to_string(),
            &Uuid::new_v4().to_string(),
            1,
        );
        for source in [
            duplicate,
            distinct.replace(&format!("role_id: \"{}\",", settings.roles[1].role_id), ""),
            distinct.replace(
                &settings.roles[1].role_id.to_string(),
                &Uuid::nil().to_string(),
            ),
            distinct.replace("infrastructure: \"local\"", "infrastructure: \"missing\""),
            distinct.replace("heartbeat_interval_secs: 5", "heartbeat_interval_secs: 0"),
            distinct.replace("worker_timeout_secs: 15", "worker_timeout_secs: 0"),
            distinct.replace("text.reverse/v1", "text.reverse/v2"),
            distinct.replace("max_concurrent_tasks: 16,", ""),
            distinct.replace("max_concurrent_tasks: 16", "max_concurrent_tasks: -1"),
            distinct.replace("max_concurrent_tasks: 1,", ""),
            distinct.replace("max_concurrent_tasks: 1", "max_concurrent_tasks: -1"),
            distinct.replace("color: auto", "color: other"),
            distinct.replace("role_id:", "session_id:"),
            "invalid RON".into(),
        ] {
            assert_ne!(source, distinct, "invalid fixture must alter the source");
            fs::write(&path, &source).unwrap();
            let error = Settings::from_file(&path).unwrap_err();
            assert!(format!("{error:#}").contains(&path.display().to_string()));
            assert_eq!(fs::read_to_string(&path).unwrap(), source);
        }
    }

    #[test]
    fn authority_concurrency_is_independent_per_role_and_does_not_rewrite_config() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("node.ron");
        let mut settings = Settings::default();
        let Role::Authority(default) = &settings.roles[0].kind else {
            panic!("expected authority")
        };
        assert_eq!(default.max_concurrent_tasks, 16);
        for limit in [0, 1, 32] {
            let mut role = settings.roles[0].clone();
            role.role_id = Uuid::new_v4();
            let Role::Authority(authority) = &mut role.kind else {
                panic!("expected authority")
            };
            authority.max_concurrent_tasks = limit;
            settings.roles.push(role);
        }
        confy::store_path(&path, &settings).unwrap();
        let source = fs::read_to_string(&path).unwrap();
        let loaded = Settings::from_file(&path).unwrap();
        let limits: Vec<_> = loaded
            .roles
            .iter()
            .filter_map(|role| match &role.kind {
                Role::Authority(authority) => Some(authority.max_concurrent_tasks),
                Role::Worker(worker) => {
                    assert_eq!(worker.max_concurrent_tasks, 1);
                    None
                }
            })
            .collect();
        assert_eq!(limits, [16, 0, 1, 32]);
        assert_eq!(fs::read_to_string(&path).unwrap(), source);
    }

    #[test]
    fn worker_concurrency_is_loaded_per_role_including_unlimited_without_rewriting() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("node.ron");
        let mut settings = Settings::default();
        let Role::Worker(default) = &settings.roles[1].kind else {
            panic!("expected default worker")
        };
        assert_eq!(default.max_concurrent_tasks, 1);
        for limit in [0, 4, 32] {
            let mut role = settings.roles[1].clone();
            role.role_id = Uuid::new_v4();
            let Role::Worker(worker) = &mut role.kind else {
                panic!("expected worker")
            };
            worker.max_concurrent_tasks = limit;
            settings.roles.push(role);
        }
        confy::store_path(&path, &settings).unwrap();
        let source = fs::read_to_string(&path).unwrap();
        let loaded = Settings::from_file(&path).unwrap();
        let limits: Vec<_> = loaded
            .roles
            .iter()
            .filter_map(|role| match &role.kind {
                Role::Worker(worker) => Some(worker.max_concurrent_tasks),
                Role::Authority(_) => None,
            })
            .collect();
        assert_eq!(limits, [1, 0, 4, 32]);
        assert_eq!(fs::read_to_string(&path).unwrap(), source);
    }

    #[test]
    fn pool_duplicates_are_deduplicated_without_rewriting() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("node.ron");
        let settings = Settings::default();
        confy::store_path(&path, &settings).unwrap();
        let source = fs::read_to_string(&path)
            .unwrap()
            .replace("pools: [", "pools: [\"default\", ");
        fs::write(&path, &source).unwrap();
        let loaded = Settings::from_file(&path).unwrap();
        let Role::Authority(authority) = &loaded.roles[0].kind else {
            panic!("expected authority")
        };
        assert_eq!(authority.pools.len(), 1);
        assert_eq!(fs::read_to_string(&path).unwrap(), source);
    }

    #[test]
    fn duplicate_ids_across_role_types_and_unknown_infrastructure_are_rejected() {
        let mut settings = Settings::default();
        settings.roles[1].role_id = settings.roles[0].role_id;
        assert!(
            settings
                .validate()
                .unwrap_err()
                .to_string()
                .contains("重复的 role_id")
        );
        settings.roles[1].role_id = Uuid::new_v4();
        settings.roles[1].infrastructure = "not configured".into();
        assert!(
            settings
                .validate()
                .unwrap_err()
                .to_string()
                .contains("不存在的基础设施")
        );
        settings.roles.clear();
        settings.validate().unwrap();
    }

    #[test]
    fn concurrent_first_starts_reuse_all_persisted_ids() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("node.ron");
        let barrier = std::sync::Barrier::new(8);
        let ids = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..8)
                .map(|_| {
                    scope.spawn(|| {
                        barrier.wait();
                        let settings = Settings::from_file(&path).unwrap();
                        (
                            settings.node_id,
                            settings
                                .roles
                                .iter()
                                .map(|role| role.role_id)
                                .collect::<Vec<_>>(),
                        )
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .collect::<Vec<_>>()
        });
        assert!(ids.iter().all(|id| *id == ids[0]));
    }

    #[test]
    fn reports_config_creation_failure_with_path() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("missing/node.ron");
        let error = Settings::from_file(&path).unwrap_err();
        assert!(error.to_string().contains(&path.display().to_string()));
    }
    #[test]
    fn deadlines_and_transport_limits_are_required_positive_and_not_written_back() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("node.ron");
        let mut settings = Settings {
            shutdown_timeout_secs: 30,
            ..Settings::default()
        };
        let Role::Authority(authority) = &mut settings.roles[0].kind else {
            panic!("authority")
        };
        authority.report_timeout_secs = 25;
        authority.input_capacity = 7;
        authority.max_concurrent_replies = 3;
        let Infrastructure::Nats {
            connect_timeout_secs,
            subscription_capacity,
            ..
        } = settings.infrastructures.get_mut("local").unwrap();
        *connect_timeout_secs = 20;
        *subscription_capacity = 128;
        confy::store_path(&path, &settings).unwrap();
        let source = fs::read_to_string(&path).unwrap();
        let loaded = Settings::from_file(&path).unwrap();
        assert_eq!(loaded.shutdown_timeout_secs, 30);
        let Role::Authority(authority) = &loaded.roles[0].kind else {
            panic!("authority")
        };
        assert_eq!(authority.report_timeout_secs, 25);
        assert_eq!(
            (authority.input_capacity, authority.max_concurrent_replies),
            (7, 3)
        );
        let Infrastructure::Nats {
            connect_timeout_secs,
            subscription_capacity,
            ..
        } = &loaded.infrastructures["local"];
        assert_eq!((*connect_timeout_secs, *subscription_capacity), (20, 128));
        assert_eq!(fs::read_to_string(&path).unwrap(), source);
        for field in [
            "shutdown_timeout_secs: 30",
            "report_timeout_secs: 25",
            "connect_timeout_secs: 20",
            "subscription_capacity: 128",
            "input_capacity: 7",
            "max_concurrent_replies: 3",
        ] {
            let name = field.split(':').next().unwrap();
            for replacement in [
                String::new(),
                format!("{name}: 0"),
                format!("{name}: -1"),
                format!("{name}: {}", u64::MAX),
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
}
