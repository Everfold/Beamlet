use crate::CapabilityError;
use std::time::Duration;

/// Worker 从取出调用到完成报告发送期间允许的在途数量.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Concurrency {
    /// 不在角色层限制数量, 仍受传输和系统资源限制.
    Unlimited,
    /// 有限正数, Worker 构造时拒绝 0.
    Limited(usize),
}
impl Default for Concurrency {
    /// 构造默认串行 Worker 并发策略。
    ///
    /// # 参数
    ///
    /// 无。
    ///
    /// # 返回值
    ///
    /// `Concurrency::Limited(1)`。
    fn default() -> Self {
        Self::Limited(1)
    }
}
impl Concurrency {
    /// 判断当前活动任务数是否允许再接收一项任务。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Concurrency` 的值；调用后转移其所有权（Copy 类型按值复制）。
    /// - `active`：当前占用执行名额的任务数量。
    ///
    /// # 返回值
    ///
    /// 无限模式始终返回 `true`；有限模式仅在 active 小于上限时返回 `true`。
    pub(crate) fn admits(self, active: usize) -> bool {
        match self {
            Self::Unlimited => true,
            Self::Limited(limit) => active < limit,
        }
    }
    /// 把并发策略转换为日志使用的数值。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Concurrency` 的值；调用后转移其所有权（Copy 类型按值复制）。
    ///
    /// # 返回值
    ///
    /// 有限模式返回上限；无限模式返回约定的零值。
    pub(crate) fn display_limit(self) -> usize {
        match self {
            Self::Unlimited => 0,
            Self::Limited(limit) => limit,
        }
    }
}

/// Worker 的运行参数, 与文件格式或消息服务器无关.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// 包含处理器等待和报告发送的并发上限.
    pub concurrency: Concurrency,
    /// 完整心跳的周期; 必须为正且能表示为单调时钟期限.
    pub heartbeat_interval: Duration,
    /// 单次心跳发送及退出刷新允许等待的最长时间.
    pub announcement_timeout: Duration,
}
impl Default for Options {
    /// 构造默认 Worker 并发与公告参数。
    ///
    /// # 参数
    ///
    /// 无。
    ///
    /// # 返回值
    ///
    /// 串行执行、心跳周期 5 秒、公告期限 1 秒的 Options。
    fn default() -> Self {
        Self {
            concurrency: Concurrency::default(),
            heartbeat_interval: Duration::from_secs(5),
            announcement_timeout: Duration::from_secs(1),
        }
    }
}
impl Options {
    /// 校验 Worker 并发、心跳和公告期限。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Options` 的值；调用后转移其所有权（Copy 类型按值复制）。
    ///
    /// # 返回值
    ///
    /// 合法返回 `Ok(())`；有限并发为零或期限非法返回 BuildError。
    pub(crate) fn validate(self) -> Result<(), BuildError> {
        if self.concurrency == Concurrency::Limited(0) {
            return Err(BuildError::ZeroConcurrency);
        }
        for (field, value) in [
            ("heartbeat_interval", self.heartbeat_interval),
            ("announcement_timeout", self.announcement_timeout),
        ] {
            if value.is_zero() || std::time::Instant::now().checked_add(value).is_none() {
                return Err(BuildError::InvalidDuration(field));
            }
        }
        Ok(())
    }
}
/// Worker 在绑定传输前的参数校验错误.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum BuildError {
    /// 必须显式提供持久角色 UUID 和本次运行的会话 UUID.
    #[error("role and session UUIDs must not be nil")]
    InvalidIdentity,
    /// 有限并发必须大于零, 无限并发使用 Unlimited.
    #[error("limited concurrency must be positive")]
    ZeroConcurrency,
    /// 正数期限无效.
    #[error("invalid duration: {0}")]
    InvalidDuration(&'static str),
    /// 能力选择无效.
    #[error(transparent)]
    Capabilities(#[from] CapabilityError),
}
