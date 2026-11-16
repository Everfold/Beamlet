//! 内置能力及启用校验集中管理, 公告与执行使用同一份配置.
use anyhow::{Result, bail, ensure};
use std::collections::BTreeSet;

pub(crate) const UPPERCASE: &str = "text.uppercase/v1";

pub(crate) fn validate(enabled: &BTreeSet<String>) -> Result<()> {
    ensure!(!enabled.is_empty(), "Worker 必须启用至少一种能力");
    for capability in enabled {
        ensure!(
            matches!(
                capability.as_str(),
                crate::text_reverse::CAPABILITY | UPPERCASE
            ),
            "未知能力: {capability}"
        );
    }
    Ok(())
}

pub(crate) fn execute(capability: &str, text: &str) -> Result<String> {
    match capability {
        crate::text_reverse::CAPABILITY => crate::text_reverse::execute(capability, text),
        UPPERCASE => Ok(text.to_uppercase()),
        _ => bail!("不支持的能力: {capability}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uppercase_preserves_whitespace_and_expands_unicode() {
        assert_eq!(
            execute(UPPERCASE, " Beamlet ß\n世界\t").unwrap(),
            " BEAMLET SS\n世界\t"
        );
        assert!(execute("text.uppercase/v2", "x").is_err());
        assert!(validate(&BTreeSet::new()).is_err());
        assert!(validate(&["unknown/v1".into()].into()).is_err());
    }
}
