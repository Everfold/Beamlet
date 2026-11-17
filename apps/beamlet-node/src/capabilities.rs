//! 内置能力及启用校验集中管理, 公告与执行使用同一份配置.
use anyhow::{Context, Result, bail};
use std::collections::BTreeSet;

/// 内置 Unicode 大写能力的版本化名称，与配置启用项和调用标识保持一致。
pub(crate) const UPPERCASE: &str = "text.uppercase/v1";

/// 注册 node 自带的文本倒序和大写处理器。
///
/// # 返回值
///
/// 成功返回能力注册表；能力注册失败时返回错误。
pub(crate) fn registered() -> Result<beamlet_worker::Capabilities> {
    let mut capabilities = beamlet_worker::Capabilities::new();
    for id in [crate::text_reverse::CAPABILITY, UPPERCASE] {
        capabilities.register(id, move |text, _context| async move {
            execute(id.into(), text)
                .await
                .map_err(beamlet_worker::ExecutionError::new)
        })?;
    }
    Ok(capabilities)
}
/// 检查 Worker 配置的启用集合是否非空且均有处理器。
///
/// # 参数
///
/// - `enabled`：需要启用的能力名称；按集合语义处理，不隐式注册未知能力。
///
/// # 返回值
///
/// 合法时返回 `Ok(())`；集合为空或存在未知能力时返回错误。
pub(crate) fn validate(enabled: &BTreeSet<String>) -> Result<()> {
    registered()?
        .validate_enabled(enabled.iter().map(String::as_str))
        .map_err(|error| match error {
            beamlet_worker::CapabilityError::Empty => {
                anyhow::anyhow!("Worker 必须启用至少一种能力")
            }
            beamlet_worker::CapabilityError::Unknown(id) => anyhow::anyhow!("未知能力: {id}"),
            error => error.into(),
        })
}

/// 按能力执行文本倒序或大写转换，将计算移交阻塞线程池。
///
/// # 参数
///
/// - `capability`：要调用或匹配的能力标识，保留精确拼写。
/// - `text`：待转换的原始文本，包含需要保留的空白字符。
///
/// # 返回值
///
/// 成功返回转换后的文本；能力不支持或计算任务取消时返回错误，计算 panic 继续向上传播。
pub(crate) async fn execute(capability: String, text: String) -> Result<String> {
    match capability.as_str() {
        crate::text_reverse::CAPABILITY | UPPERCASE => {
            let span = tracing::Span::current();
            let task = tokio::task::spawn_blocking(move || {
                let _entered = span.enter();
                match capability.as_str() {
                    crate::text_reverse::CAPABILITY => {
                        crate::text_reverse::execute(&capability, &text)
                    }
                    UPPERCASE => Ok(text.to_uppercase()),
                    _ => unreachable!("只有已匹配的文本能力进入计算任务"),
                }
            });
            match task.await {
                Ok(result) => result,
                // 计算 panic 仍由角色监督处理, 不伪装成普通业务拒绝.
                Err(error) if error.is_panic() => std::panic::resume_unwind(error.into_panic()),
                Err(error) => Err(error).context("能力计算任务异常退出"),
            }
        }
        _ => bail!("不支持的能力: {capability}"),
    }
}

/// 当前模块关键行为与边界条件的单元测试，实现在 tests/unit 目录。
#[cfg(test)]
#[path = "../tests/unit/capabilities.rs"]
mod tests;
