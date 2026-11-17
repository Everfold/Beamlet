//! 文本倒写的能力标识、版本校验与业务行为, 不依赖交付或生命周期.

use anyhow::{Result, ensure};

/// v1 接受并返回 UTF-8 文本, 按 Unicode 标量值倒写.
pub(crate) const CAPABILITY: &str = "text.reverse/v1";

/// 按 Unicode 标量倒序文本，保留所有空白字符。
///
/// # 参数
///
/// - `capability`：要调用或匹配的能力标识，保留精确拼写。
/// - `text`：待转换的原始文本，包含需要保留的空白字符。
///
/// # 返回值
///
/// 能力受支持时返回倒序字符串；不支持的能力或版本返回错误。
///
/// # Errors
/// 仅接受精确的能力标识与版本, 不隐式兼容其他版本.
pub(crate) fn execute(capability: &str, text: &str) -> Result<String> {
    ensure!(capability == CAPABILITY, "不支持的能力: {capability}");

    Ok(text.chars().rev().collect())
}

/// 当前模块关键行为与边界条件的单元测试，实现在 tests/unit 目录。
#[cfg(test)]
#[path = "../tests/unit/text_reverse.rs"]
mod tests;
