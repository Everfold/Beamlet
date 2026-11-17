use std::time::Duration;

/// Authority 同时交付并等待报告的数量.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Concurrency {
    /// 不在角色层限制数量, 仍受传输和系统资源限制.
    Unlimited,
    /// 有限正数, Authority 构造时拒绝 0.
    Limited(usize),
}
impl Default for Concurrency {
    /// 构造 Authority 默认的有限任务并发。
    ///
    /// # 返回值
    ///
    /// `Concurrency::Limited(16)`。
    fn default() -> Self {
        Self::Limited(16)
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

/// Authority 的运行参数, 文件配置由调用方转换.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// 同时交付并等待报告的上限.
    pub concurrency: Concurrency,
    /// 待发送业务结果/拒绝回复的上限, 必须为正.
    pub max_concurrent_replies: usize,
    /// 远端业务提交队列容量, 必须为正.
    pub input_capacity: usize,
    /// Worker 完整公告的存活期限.
    pub worker_timeout: Duration,
    /// 单次交付等待报告的期限.
    pub report_timeout: Duration,
}
impl Default for Options {
    /// 构造默认 Authority 运行参数。
    ///
    /// # 返回值
    ///
    /// 任务/回复/输入容量各 16，Worker 超时 15 秒，报告等待 3 秒的 Options。
    fn default() -> Self {
        Self {
            concurrency: Concurrency::default(),
            max_concurrent_replies: 16,
            input_capacity: 16,
            worker_timeout: Duration::from_secs(15),
            report_timeout: Duration::from_secs(3),
        }
    }
}
impl Options {
    /// 校验 Authority 并发、队列容量和期限。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Options` 的值；调用后转移其所有权（Copy 类型按值复制）。
    ///
    /// # 返回值
    ///
    /// 合法返回 `Ok(())`；有限并发为零、容量或时钟期限非法时返回 BuildError。
    pub(crate) fn validate(self) -> Result<(), BuildError> {
        if self.concurrency == Concurrency::Limited(0) {
            return Err(BuildError::ZeroConcurrency);
        }
        for (name, value) in [
            ("max_concurrent_replies", self.max_concurrent_replies),
            ("input_capacity", self.input_capacity),
        ] {
            if value == 0 || value > tokio::sync::Semaphore::MAX_PERMITS {
                return Err(BuildError::InvalidCapacity(name));
            }
        }
        for (name, value) in [
            ("worker_timeout", self.worker_timeout),
            ("report_timeout", self.report_timeout),
        ] {
            if value.is_zero() || std::time::Instant::now().checked_add(value).is_none() {
                return Err(BuildError::InvalidDuration(name));
            }
        }
        Ok(())
    }
}
/// 构造 Authority 时拒绝的参数.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum BuildError {
    /// 角色或会话 UUID 为空.
    #[error("role and session UUIDs must not be nil")]
    InvalidIdentity,
    /// 未加入任何 pool.
    #[error("authority pools must not be empty")]
    EmptyPools,
    /// 有限并发必须为正数.
    #[error("limited concurrency must be positive")]
    ZeroConcurrency,
    /// 容量为零或超出通道可表示范围.
    #[error("invalid capacity: {0}")]
    InvalidCapacity(&'static str),
    /// 期限为零或超出时钟可表示范围.
    #[error("invalid duration: {0}")]
    InvalidDuration(&'static str),
}
