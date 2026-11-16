//! 文本倒写的能力标识、版本校验与业务行为, 不依赖交付或生命周期.

use anyhow::{Result, ensure};

/// v1 接受并返回 UTF-8 文本, 按 Unicode 标量值倒写.
pub(crate) const CAPABILITY: &str = "text.reverse/v1";

/// 保留全部字符, 不修剪空白, 不保持组合字形或复合 emoji.
///
/// # Errors
/// 仅接受精确的能力标识与版本, 不隐式兼容其他版本.
pub(crate) fn execute(capability: &str, text: &str) -> Result<String> {
    ensure!(capability == CAPABILITY, "不支持的能力: {capability}");

    Ok(text.chars().rev().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reverses_text_without_trimming() {
        for (input, expected) in [
            ("", ""),
            ("abc", "cba"),
            (" Hello 世界\n🙂", "🙂\n界世 olleH "),
            ("\t a \r\n", "\n\r a \t"),
            ("a\0b", "b\0a"),
        ] {
            // 使用既有线协议的标识, 防止实现与声明一起变动时漏检版本变化.
            assert_eq!(
                execute("text.reverse/v1", input).unwrap(),
                expected,
                "输入: {input:?}"
            );
        }
    }

    #[test]
    fn reverses_scalars_not_graphemes() {
        for (input, expected) in [
            ("e\u{301}", "\u{301}e"),
            ("👩\u{200d}💻", "💻\u{200d}👩"),
            ("\u{1f1e8}\u{1f1f3}", "\u{1f1f3}\u{1f1e8}"),
        ] {
            assert_eq!(
                execute(CAPABILITY, input).unwrap(),
                expected,
                "输入: {input:?}"
            );
        }
    }

    #[test]
    fn rejects_other_capabilities_and_versions() {
        for capability in [
            "",
            "text.reverse",
            "text.reverse/v2",
            "TEXT.REVERSE/v1",
            "text.reverse/v1 ",
            "text.uppercase/v1",
        ] {
            let error = execute(capability, "abc").unwrap_err();
            assert_eq!(error.to_string(), format!("不支持的能力: {capability}"));
        }
    }
}
