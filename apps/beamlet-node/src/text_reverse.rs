//! 文本倒序能力及其版本标识.

/// v1 接受并返回 UTF-8 文本, 按 Unicode 标量值倒写.
pub(crate) const CAPABILITY: &str = "text.reverse/v1";

/// 返回 text 按 Unicode 标量倒序后的字符串, 保留空白字符.
pub(crate) fn reverse(text: &str) -> String {
    text.chars().rev().collect()
}

/// 当前模块关键行为与边界条件的单元测试, 实现在 tests/unit 目录.
#[cfg(test)]
#[path = "../tests/unit/text_reverse.rs"]
mod tests;
