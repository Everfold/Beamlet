//! 内置能力及启用校验集中管理, 公告与执行使用同一份配置.
use anyhow::{Context, Result, bail};
use std::collections::BTreeSet;

pub(crate) const UPPERCASE: &str = "text.uppercase/v1";

/// 示例能力由应用注册, 库不包含文本白名单或处理规则.
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

/// 能力自行选择异步 I/O 或阻塞计算, Worker 只等待执行结果.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn uppercase_preserves_whitespace_and_expands_unicode() {
        assert_eq!(
            execute(UPPERCASE.into(), " Beamlet ß\n世界\t".into())
                .await
                .unwrap(),
            " BEAMLET SS\n世界\t"
        );
        assert!(
            execute("text.uppercase/v2".into(), "x".into())
                .await
                .is_err()
        );
        assert!(validate(&BTreeSet::new()).is_err());
        assert!(validate(&["unknown/v1".into()].into()).is_err());
    }
}
