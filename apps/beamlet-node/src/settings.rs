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

/// 终端 ANSI 颜色策略, 由应用初始化日志输出时解释.
#[derive(Debug, Default, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Color {
    /// 根据终端能力和 NO_COLOR 环境变量自动选择颜色.
    #[default]
    Auto,
    /// 总是输出 ANSI 颜色, 即使输出不是终端.
    Always,
    /// 禁用 ANSI 颜色.
    Never,
}
impl Color {
    /// 根据颜色配置, 终端检测和 NO_COLOR 决定是否启用 ANSI 颜色.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Color` 的值; 调用后转移其所有权(Copy 类型按值复制).
    ///
    /// # 返回值
    ///
    /// 应该输出颜色时返回 `true`, 否则返回 `false`.
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

/// Node 支持的基础设施连接配置; 映射中每个名称对应一个独立实例.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Infrastructure {
    /// 创建独立的进程内消息空间.
    Local {
        /// 每个本地有界通道的容量, 必须大于零.
        capacity: usize,
    },
    /// 建立一个 NATS 客户端连接, 不自动启动消息服务器.
    Nats {
        /// 该命名 NATS 实例连接的服务地址.
        url: String,
        /// 首次连接的最长等待时长, 单位为秒, 必须为正.
        connect_timeout_secs: u64,
        /// 每个 NATS 订阅的有界缓冲容量, 不是所有订阅共享的总量.
        subscription_capacity: usize,
    },
}

/// 单个 Authority 的文件配置, 独立设置 pools, 租约, 报告期限与并发容量.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Authority {
    /// 本 Authority 加入的非空 pool 集合; 重复项按集合语义去重.
    pub pools: BTreeSet<Pool>,
    /// Worker 完整公告的有效租约时长, 单位为秒.
    pub worker_timeout_secs: u64,
    /// 单次交付等待 Worker Report 的最长时长, 单位为秒.
    pub report_timeout_secs: u64,
    /// 各传输入口共用的业务队列容量.
    pub input_capacity: usize,
    /// 等待发送的结果/拒绝回复上限, 必须大于 0.
    pub max_concurrent_replies: usize,
    /// 同时交付并等待报告的任务上限; 0 不限制.
    pub max_concurrent_tasks: usize,
}
impl From<&Authority> for beamlet_authority::Options {
    /// 把 Authority 文件配置转换为有单位的运行参数.
    ///
    /// # 参数
    ///
    /// - `config`: 应用层文件配置, 包含数值形式的期限和并发上限.
    ///
    /// # 返回值
    ///
    /// Authority Options; 任务上限零转换为 Unlimited, 其余期限转换为 `Duration`.
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
    /// 构造新 Authority 配置的默认 pools, 期限及并发容量.
    ///
    /// # 返回值
    ///
    /// 默认 Authority 文件配置, 不生成角色身份.
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

/// 单个 Worker 的文件配置, 独立设置 pool, 启用能力, 心跳和并发上限.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Worker {
    /// 该 Worker 唯一所属的 pool.
    pub pool: Pool,
    /// 该角色实际注册并执行的能力名称集合, 不能为空.
    pub capabilities: BTreeSet<String>,
    /// 发送周期性完整心跳公告的间隔, 单位为秒.
    pub heartbeat_interval_secs: u64,
    /// 同时执行并发送报告的任务上限; 0 不限制.
    pub max_concurrent_tasks: usize,
}
impl From<&Worker> for beamlet_worker::Options {
    /// 把 Worker 文件配置转换为执行与心跳参数.
    ///
    /// # 参数
    ///
    /// - `config`: 应用层文件配置, 包含数值形式的期限和并发上限.
    ///
    /// # 返回值
    ///
    /// Worker Options; 任务上限零转换为 Unlimited, 心跳秒数转换为 `Duration`.
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
    /// 构造单个 Worker 的默认 pool, 能力和执行参数.
    ///
    /// # 返回值
    ///
    /// 启用文本倒序能力, 使用默认心跳与串行执行的配置.
    fn default() -> Self {
        Self {
            pool: Pool::default(),
            capabilities: BTreeSet::from([crate::text_reverse::CAPABILITY.into()]),
            heartbeat_interval_secs: 5,
            max_concurrent_tasks: 1,
        }
    }
}

/// 一个角色的种类及其专属配置; 同一 Node 可装配多个相同种类的角色.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Role {
    /// 管理 Operation, 登记 Worker 并签发执行授权的协调角色.
    Authority(
        /// 当前 Authority 的专属配置.
        Authority,
    ),
    /// 执行收到的授权并创建 Report 的工作角色.
    Worker(
        /// 当前 Worker 的专属配置.
        Worker,
    ),
}
impl Role {
    /// 取得角色种类的稳定名称, 用于日志.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Role` 的共享引用.
    ///
    /// # 返回值
    ///
    /// `authority` 或 `worker` 静态字符串.
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Authority(_) => "authority",
            Self::Worker(_) => "worker",
        }
    }
}

/// 角色持久配置与基础设施引用, 不保存每次运行生成的会话 UUID.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RoleSettings {
    /// 角色的持久 UUID; 已有配置必须提供且同一 Node 内不能重复.
    pub role_id: Uuid,
    /// 可选的人类可读标签, 不参与身份比较或消息路由.
    pub label: Option<String>,
    /// 引用的基础设施名称; Authority 可多选, Worker 只能选一个.
    pub infrastructures: BTreeSet<String>,
    /// 角色种类及其独立配置.
    pub kind: Role,
}

/// Node 的 RON 配置布局, 组合固定节点身份, 命名基础设施和多个角色.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Settings {
    /// Node 的持久 UUID, 仅用于宿主识别和日志, 不作为任务路由目标.
    pub node_id: Uuid,
    /// 可选的人类可读标签, 不参与身份比较或消息路由.
    pub label: Option<String>,
    /// 整个应用的终端日志颜色策略.
    pub color: Color,
    /// 全体角色从收到停止通知到排空结束的总期限, 单位为秒.
    pub shutdown_timeout_secs: u64,
    /// 名称到基础设施配置的映射; 同名引用共享连接, 不同名称独立连接.
    pub infrastructures: BTreeMap<String, Infrastructure>,
    /// 需要装配的角色列表; 可为空, 也可包含多个同类型角色.
    pub roles: Vec<RoleSettings>,
}
impl Default for Settings {
    /// 构造首次启动 Node 时的默认基础设施与两个角色.
    ///
    /// # 返回值
    ///
    /// 具有新 Node UUID 和两个不同角色 UUID 的配置; 不包含运行会话 UUID.
    fn default() -> Self {
        Self {
            node_id: Uuid::new_v4(),
            label: None,
            color: Color::Auto,
            shutdown_timeout_secs: 6,
            infrastructures: BTreeMap::from([(
                "nats".into(),
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
                    infrastructures: ["nats".into()].into(),
                    kind: Role::Authority(Authority::default()),
                },
                RoleSettings {
                    role_id: Uuid::new_v4(),
                    label: Some("worker".into()),
                    infrastructures: ["nats".into()].into(),
                    kind: Role::Worker(Worker::default()),
                },
            ],
        }
    }
}
impl Settings {
    /// 定位并加载可执行文件旁的指定配置文件.
    ///
    /// # 参数
    ///
    /// - `file_name`: 可执行文件旁的单个文件名, 如 node.ron; 路径或目录名会被拒绝.
    ///
    /// # 返回值
    ///
    /// 成功返回已校验配置; 文件名非法, 可执行路径获取或加载失败时返回错误.
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
    /// 从指定 RON 文件加载并校验配置, 文件缺失时首次创建.
    ///
    /// # 参数
    ///
    /// - `path`: 配置文件路径; 不存在时由 confy 首次创建, 已有文件不迁移或重写.
    ///
    /// # 返回值
    ///
    /// 成功返回配置; 读取, 首次写入, 解析或语义校验失败时返回带文件路径的错误.
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
    /// 校验节点身份, 基础设施引用及各角色独立配置.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Settings` 的共享引用.
    ///
    /// # 返回值
    ///
    /// 所有约束满足时返回 `Ok(())`; 重复/无效身份, 未知引用或非法参数返回错误.
    pub(crate) fn validate(&self) -> Result<()> {
        validate_timeout(self.shutdown_timeout_secs, "shutdown_timeout_secs")?;
        ensure!(!self.node_id.is_nil(), "node_id 不能是 nil UUID");
        ensure!(!self.infrastructures.is_empty(), "至少需要一个基础设施实例");
        for (name, infrastructure) in &self.infrastructures {
            ensure!(!name.trim().is_empty(), "基础设施名称不能为空");
            match infrastructure {
                Infrastructure::Local { capacity } => validate_capacity(*capacity, "capacity")?,
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
                !role.infrastructures.is_empty(),
                "角色 {} 至少需要一个基础设施",
                role.role_id
            );
            for name in &role.infrastructures {
                ensure!(
                    self.infrastructures.contains_key(name),
                    "角色 {} 引用了不存在的基础设施: {name}",
                    role.role_id
                );
            }
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
                    validate_timeout(config.worker_timeout_secs, "worker_timeout_secs")?;
                }
                Role::Worker(config) => {
                    ensure!(
                        role.infrastructures.len() == 1,
                        "Worker {} 只能引用一个基础设施",
                        role.role_id
                    );
                    validate_timeout(config.heartbeat_interval_secs, "heartbeat_interval_secs")?;
                    crate::capabilities::validate(&config.capabilities)
                        .with_context(|| format!("Worker {} 能力配置无效", role.role_id))?;
                }
            }
        }
        Ok(())
    }
}

/// 校验按秒表示的期限非零且能由单调时钟表示.
///
/// # 参数
///
/// - `seconds`: 待校验的期限, 单位为秒.
/// - `name`: 用于校验错误提示的配置字段名.
///
/// # 返回值
///
/// 合法返回 `Ok(())`; 零值或时钟溢出返回字段错误.
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
/// 校验有界队列容量处于 Tokio 可表示范围内.
///
/// # 参数
///
/// - `capacity`: 有界接收队列的容量, 必须大于零且不超过 Tokio 最大许可数.
/// - `name`: 用于校验错误提示的配置字段名.
///
/// # 返回值
///
/// 合法返回 `Ok(())`; 零值或超出最大许可数返回字段错误.
fn validate_capacity(capacity: usize, name: &str) -> Result<()> {
    ensure!(
        capacity > 0 && capacity <= tokio::sync::Semaphore::MAX_PERMITS,
        "{name} 必须大于 0 且不超过通道容量范围"
    );
    Ok(())
}

/// 当前模块关键行为与边界条件的单元测试, 实现在 tests/unit 目录.
#[cfg(test)]
#[path = "../tests/unit/settings.rs"]
mod tests;
