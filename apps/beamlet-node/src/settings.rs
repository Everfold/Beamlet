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
    /// 根据颜色配置、终端检测和 NO_COLOR 决定是否启用 ANSI 颜色。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Color` 的值；调用后转移其所有权（Copy 类型按值复制）。
    ///
    /// # 返回值
    ///
    /// 应该输出颜色时返回 `true`，否则返回 `false`。
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
impl From<&Authority> for beamlet_authority::Options {
    /// 把 Authority 文件配置转换为有单位的运行参数。
    ///
    /// # 参数
    ///
    /// - `config`：应用层文件配置，包含数值形式的期限和并发上限。
    ///
    /// # 返回值
    ///
    /// Authority Options；任务上限零转换为 Unlimited，其余期限转换为 `Duration`。
    fn from(config: &Authority) -> Self {
        Self {
            concurrency: match config.max_concurrent_tasks {
                0 => beamlet_authority::Concurrency::Unlimited,
                limit => beamlet_authority::Concurrency::Limited(limit),
            },
            max_concurrent_replies: config.max_concurrent_replies,
            input_capacity: config.input_capacity,
            worker_timeout: std::time::Duration::from_secs(config.worker_timeout_secs),
            report_timeout: std::time::Duration::from_secs(config.report_timeout_secs),
        }
    }
}
impl Default for Authority {
    /// 构造新 Authority 配置的默认 pools、期限及并发容量。
    ///
    /// # 返回值
    ///
    /// 默认 Authority 文件配置，不生成角色身份。
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
impl From<&Worker> for beamlet_worker::Options {
    /// 把 Worker 文件配置转换为执行与心跳参数。
    ///
    /// # 参数
    ///
    /// - `config`：应用层文件配置，包含数值形式的期限和并发上限。
    ///
    /// # 返回值
    ///
    /// Worker Options；任务上限零转换为 Unlimited，心跳秒数转换为 `Duration`。
    fn from(config: &Worker) -> Self {
        Self {
            concurrency: match config.max_concurrent_tasks {
                0 => beamlet_worker::Concurrency::Unlimited,
                limit => beamlet_worker::Concurrency::Limited(limit),
            },
            heartbeat_interval: std::time::Duration::from_secs(config.heartbeat_interval_secs),
            ..Self::default()
        }
    }
}
impl Default for Worker {
    /// 构造单个 Worker 的默认 pool、能力和执行参数。
    ///
    /// # 返回值
    ///
    /// 启用文本倒序能力、使用默认心跳与串行执行的配置。
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
    /// 取得角色种类的稳定名称，用于日志。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Role` 的共享引用。
    ///
    /// # 返回值
    ///
    /// `authority` 或 `worker` 静态字符串。
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
    /// 构造首次启动 Node 时的默认基础设施与两个角色。
    ///
    /// # 返回值
    ///
    /// 具有新 Node UUID 和两个不同角色 UUID 的配置；不包含运行会话 UUID。
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
    /// 定位并加载可执行文件旁的指定配置文件。
    ///
    /// # 参数
    ///
    /// - `file_name`：可执行文件旁的单个文件名，如 node.ron；路径或目录名会被拒绝。
    ///
    /// # 返回值
    ///
    /// 成功返回已校验配置；文件名非法、可执行路径获取或加载失败时返回错误。
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
    /// 从指定 RON 文件加载并校验配置，文件缺失时首次创建。
    ///
    /// # 参数
    ///
    /// - `path`：配置文件路径；不存在时由 confy 首次创建，已有文件不迁移或重写。
    ///
    /// # 返回值
    ///
    /// 成功返回配置；读取、首次写入、解析或语义校验失败时返回带文件路径的错误。
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
    /// 校验节点身份、基础设施引用及各角色独立配置。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Settings` 的共享引用。
    ///
    /// # 返回值
    ///
    /// 所有约束满足时返回 `Ok(())`；重复/无效身份、未知引用或非法参数返回错误。
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

/// 校验按秒表示的期限非零且能由单调时钟表示。
///
/// # 参数
///
/// - `seconds`：待校验的期限，单位为秒。
/// - `name`：用于校验错误提示的配置字段名。
///
/// # 返回值
///
/// 合法返回 `Ok(())`；零值或时钟溢出返回字段错误。
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
/// 校验有界队列容量处于 Tokio 可表示范围内。
///
/// # 参数
///
/// - `capacity`：有界接收队列的容量，必须大于零且不超过 Tokio 最大许可数。
/// - `name`：用于校验错误提示的配置字段名。
///
/// # 返回值
///
/// 合法返回 `Ok(())`；零值或超出最大许可数返回字段错误。
fn validate_capacity(capacity: usize, name: &str) -> Result<()> {
    ensure!(
        capacity > 0 && capacity <= tokio::sync::Semaphore::MAX_PERMITS,
        "{name} 必须大于 0 且不超过通道容量范围"
    );
    Ok(())
}

#[cfg(test)]
#[path = "../tests/unit/settings.rs"]
mod tests;
