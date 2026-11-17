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
#[path = "../tests/unit/text_reverse.rs"]
mod tests;
