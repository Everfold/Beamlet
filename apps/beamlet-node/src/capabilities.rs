//! 内置能力及启用校验集中管理, 公告与执行使用同一份配置.
use anyhow::{Context, Result};
use beamlet_worker::ExecutionError;
use std::collections::BTreeSet;

/// 内置 Unicode 大写能力的版本化名称, 与配置启用项和调用标识保持一致.
pub(crate) const UPPERCASE: &str = "text.uppercase/v1";

/// 注册 node 自带的文本倒序和大写处理器.
///
/// # 返回值
///
/// 成功返回能力注册表; 能力注册失败时返回错误.
pub(crate) fn registered() -> Result<beamlet_worker::Capabilities> {
    let mut capabilities = beamlet_worker::Capabilities::new();
    capabilities.register(crate::text_reverse::CAPABILITY, |text, _| {
        run_blocking(crate::text_reverse::reverse, text)
    })?;
    capabilities.register(UPPERCASE, |text, _| run_blocking(str::to_uppercase, text))?;
    Ok(capabilities)
}
/// 检查 Worker 配置的启用集合是否非空且均有处理器.
///
/// # 参数
///
/// - `builtins`: 节点已注册的内置能力.
/// - `enabled`: 需要启用的能力名称; 按集合语义处理, 不隐式注册未知能力.
///
/// # 返回值
///
/// 合法时返回 `Ok(())`; 集合为空或存在未知能力时返回错误.
pub(crate) fn validate(
    builtins: &beamlet_worker::Capabilities,
    enabled: &BTreeSet<String>,
) -> Result<()> {
    builtins
        .validate_enabled(enabled.iter().map(String::as_str))
        .map_err(|error| match error {
            beamlet_worker::CapabilityError::Empty => {
                anyhow::anyhow!("Worker 必须启用至少一种能力")
            }
            beamlet_worker::CapabilityError::Unknown(id) => anyhow::anyhow!("未知能力: {id}"),
            error => error.into(),
        })
}

/// 在线程池中用 transform 转换 text, 返回结果或任务取消错误, 并传播 panic.
async fn run_blocking(
    transform: fn(&str) -> String,
    text: String,
) -> Result<String, ExecutionError> {
    let span = tracing::Span::current();
    match tokio::task::spawn_blocking(move || span.in_scope(|| transform(&text))).await {
        Ok(result) => Ok(result),
        Err(error) if error.is_panic() => std::panic::resume_unwind(error.into_panic()),
        Err(error) => Err(error)
            .context("能力计算任务异常退出")
            .map_err(ExecutionError::new),
    }
}

/// 当前模块关键行为与边界条件的单元测试, 实现在 tests/unit 目录.
#[cfg(test)]
#[path = "../tests/unit/capabilities.rs"]
mod tests;
