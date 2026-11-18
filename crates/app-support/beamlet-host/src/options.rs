use std::time::{Duration, Instant};

/// 宿主参数, 不包含角色或传输的配置.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// 广播停止后等待所有任务结束的总期限, 必须为正且可由时钟表示.
    pub shutdown_timeout: Duration,
}
impl Default for Options {
    /// 构造默认宿主退出期限.
    ///
    /// # 返回值
    ///
    /// 退出排空期限为 6 秒的 Options.
    fn default() -> Self {
        Self {
            shutdown_timeout: Duration::from_secs(6),
        }
    }
}
impl Options {
    /// 校验宿主退出期限为正且可由单调时钟表示.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Options` 的值; 调用后转移其所有权(Copy 类型按值复制).
    ///
    /// # 返回值
    ///
    /// 合法返回 `Ok(())`; 零值或溢出返回 BuildError.
    pub(crate) fn validate(self) -> Result<(), BuildError> {
        if self.shutdown_timeout.is_zero()
            || Instant::now().checked_add(self.shutdown_timeout).is_none()
        {
            Err(BuildError)
        } else {
            Ok(())
        }
    }
}

/// 排空期限为零或超出时钟表示范围.
#[derive(Debug, thiserror::Error)]
#[error("shutdown timeout must be positive and representable by the clock")]
pub struct BuildError;
