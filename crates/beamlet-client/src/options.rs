use std::time::{Duration, Instant};

/// 客户端调用期限, 由使用方从任意配置来源转换.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// 一次调用的总等待期限, 包括发现入口创建与回复收集.
    pub request_timeout: Duration,
    /// 发现入口就绪后收集多份回复的窗口, 必须小于总等待期限.
    pub discovery_window: Duration,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            request_timeout: Duration::from_secs(10),
            discovery_window: Duration::from_secs(1),
        }
    }
}
impl Options {
    pub(crate) fn validate(self) -> Result<(), BuildError> {
        for (name, duration) in [
            ("request_timeout", self.request_timeout),
            ("discovery_window", self.discovery_window),
        ] {
            if duration.is_zero() || Instant::now().checked_add(duration).is_none() {
                return Err(BuildError::InvalidDuration(name));
            }
        }
        if self.discovery_window >= self.request_timeout {
            return Err(BuildError::DiscoveryWindowTooLong);
        }
        Ok(())
    }
}

/// 创建客户端时拒绝的运行参数.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum BuildError {
    /// 期限为零或超出时钟表示范围.
    #[error("invalid duration: {0}")]
    InvalidDuration(&'static str),
    /// 总期限需要给发现入口创建留出时间.
    #[error("discovery window must be shorter than request timeout")]
    DiscoveryWindowTooLong,
}
