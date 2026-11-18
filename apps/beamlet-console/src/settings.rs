use anyhow::{Context, Result, ensure};
use beamlet_api::Pool;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::{env, io::IsTerminal, path::Path};

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

/// Console 的 RON 配置布局, 加载后分别转换为传输和 Client 的运行参数.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Settings {
    /// 整个应用的日志颜色策略.
    pub color: Color,
    /// NATS 地址和订阅配置.
    pub nats: NatsSettings,
    /// 发现范围, 请求期限及在途上限.
    pub client: ClientSettings,
}

/// Console 使用的 NATS 连接.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NatsSettings {
    /// NATS 服务地址.
    pub url: String,
    /// 首次连接期限, 单位为秒.
    pub connect_timeout_secs: u64,
    /// 每个订阅的接收缓冲容量.
    pub subscription_capacity: usize,
}

/// Console 的发现与请求配置.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ClientSettings {
    /// 允许发现和选择的 pool.
    pub pools: BTreeSet<Pool>,
    /// 单次请求总期限, 单位为秒.
    pub request_timeout_secs: u64,
    /// 发现回复收集窗口, 单位为毫秒.
    pub discovery_window_ms: u64,
    /// 同时在途的发现, 查询和提交总数.
    pub max_in_flight_requests: usize,
}

impl From<&ClientSettings> for beamlet_client::Options {
    /// 将 Console 文件配置中的期限转换为 Client 运行参数.
    ///
    /// # 参数
    ///
    /// - `settings`: 发现窗口与请求期限.
    ///
    /// # 返回值
    ///
    /// 以 `Duration` 表达请求期限和发现窗口的 Options; 不修改配置或执行校验.
    fn from(settings: &ClientSettings) -> Self {
        Self {
            request_timeout: std::time::Duration::from_secs(settings.request_timeout_secs),
            discovery_window: std::time::Duration::from_millis(settings.discovery_window_ms),
        }
    }
}

impl beamlet_app_config::Config for Settings {
    const COMMENTS: &[(&str, &str)] = &[
        ("color", "终端颜色 / Terminal color (auto | always | never)"),
        ("nats.url", "NATS 服务地址 / NATS server URL"),
        (
            "nats.connect_timeout_secs",
            "首次连接超时(秒) / Initial connection timeout (s)",
        ),
        (
            "nats.subscription_capacity",
            "每个订阅缓冲的消息数 / Buffered messages per subscription",
        ),
        (
            "client.pools",
            "发现 Authority 的池 / Pools to search for Authorities",
        ),
        (
            "client.request_timeout_secs",
            "单次请求总超时(秒) / Overall request timeout (s)",
        ),
        (
            "client.discovery_window_ms",
            "收集发现回复的毫秒数, 须短于请求超时 / Discovery window (ms), shorter than request timeout",
        ),
        (
            "client.max_in_flight_requests",
            "并发请求数, 至少 1 / Concurrent requests, minimum 1",
        ),
    ];
}

impl Default for Settings {
    /// 构造首次创建 Console 配置时使用的默认值.
    ///
    /// # 返回值
    ///
    /// 包含默认 NATS 地址, pool, 颜色和各项期限/容量的配置.
    fn default() -> Self {
        Self {
            color: Color::Auto,
            nats: NatsSettings {
                url: "nats://127.0.0.1:4222".into(),
                connect_timeout_secs: 10,
                subscription_capacity: 65536,
            },
            client: ClientSettings {
                pools: BTreeSet::from([Pool::default()]),
                request_timeout_secs: 10,
                discovery_window_ms: 1000,
                max_in_flight_requests: 16,
            },
        }
    }
}

impl Settings {
    /// 加载可执行文件旁的 console.ron, 缺失时首次创建.
    ///
    /// # 返回值
    ///
    /// 成功返回已校验配置; 路径获取, 配置读写或校验失败时返回错误.
    pub(crate) fn load() -> Result<Self> {
        let path = env::current_exe()
            .context("获取可执行文件路径失败")?
            .with_file_name("console.ron");
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
        let settings: Self = beamlet_app_config::load(path)
            .with_context(|| format!("加载控制台配置失败: {}", path.display()))?;
        settings
            .validate()
            .with_context(|| format!("控制台配置无效: {}", path.display()))?;
        Ok(settings)
    }
    /// 校验 Console 的发现范围, 地址, 期限和容量.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Settings` 的共享引用.
    ///
    /// # 返回值
    ///
    /// 全部合法时返回 `Ok(())`; 任一约束不满足时返回带字段信息的错误.
    pub(crate) fn validate(&self) -> Result<()> {
        ensure!(!self.client.pools.is_empty(), "Console 至少需要一个 pool");
        ensure!(!self.nats.url.trim().is_empty(), "nats.url 不能为空");
        for (name, seconds) in [
            ("request_timeout_secs", self.client.request_timeout_secs),
            ("connect_timeout_secs", self.nats.connect_timeout_secs),
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
            self.client.discovery_window_ms > 0
                && std::time::Duration::from_millis(self.client.discovery_window_ms)
                    < std::time::Duration::from_secs(self.client.request_timeout_secs),
            "discovery_window_ms 必须大于 0 且小于 request_timeout_secs 指定的期限"
        );
        ensure!(
            self.client.max_in_flight_requests > 0,
            "max_in_flight_requests 必须大于 0"
        );
        ensure!(
            self.nats.subscription_capacity > 0
                && self.nats.subscription_capacity <= tokio::sync::Semaphore::MAX_PERMITS,
            "subscription_capacity 必须大于 0 且不超过通道容量范围"
        );
        Ok(())
    }
}

/// 当前模块关键行为与边界条件的单元测试, 实现在 tests/unit 目录.
#[cfg(test)]
#[path = "../tests/unit/settings.rs"]
mod tests;
