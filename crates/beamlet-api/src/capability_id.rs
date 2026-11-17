use std::{fmt, str::FromStr};

/// 带版本的能力名称, 长度为 1..=256 字节, 不自动修改大小写或空白.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "String", into = "String"))]
pub struct CapabilityId(
    /// 已校验且保留精确拼写的能力名称，长度为 1..=256 字节。
    String,
);

/// 能力标识为空或超过允许的字节长度.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("capability id must contain 1..=256 bytes")]
pub struct InvalidCapabilityId;

impl CapabilityId {
    /// 借用经过校验的能力标识文本。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `CapabilityId` 的共享引用。
    ///
    /// # 返回值
    ///
    /// 原始能力字符串的切片，不修剪或重新编码。
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// 校验能力标识的非空和字节长度限制。
    ///
    /// # 参数
    ///
    /// - `value`：待校验的能力名称，要求 1..=256 字节，不修剪空白。
    ///
    /// # 返回值
    ///
    /// 合法时返回 `Ok(())`；为空或超过 256 字节时返回 InvalidCapabilityId。
    ///
    /// # Errors
    /// 空标识或长度超过 256 字节时返回错误.
    pub fn validate(value: &str) -> Result<(), InvalidCapabilityId> {
        if value.is_empty() || value.len() > 256 {
            Err(InvalidCapabilityId)
        } else {
            Ok(())
        }
    }
}

impl TryFrom<String> for CapabilityId {
    /// 能力名称为空或超过字节长度上限时返回的校验错误。
    type Error = InvalidCapabilityId;
    /// 校验并保存能力标识，保留输入的精确拼写。
    ///
    /// # 参数
    ///
    /// - `value`：待接收所有权的能力名称，要求 1..=256 字节。
    ///
    /// # 返回值
    ///
    /// 成功返回 CapabilityId；为空或超过 256 字节时返回错误。
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::validate(&value)?;
        Ok(Self(value))
    }
}
impl FromStr for CapabilityId {
    /// 能力名称为空或超过字节长度上限时返回的校验错误。
    type Err = InvalidCapabilityId;
    /// 校验并保存能力标识，保留输入的精确拼写。
    ///
    /// # 参数
    ///
    /// - `value`：待校验的能力名称，要求 1..=256 字节，不修剪空白。
    ///
    /// # 返回值
    ///
    /// 成功返回 CapabilityId；为空或超过 256 字节时返回错误。
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::validate(value)?;
        Ok(Self(value.to_owned()))
    }
}
impl From<CapabilityId> for String {
    /// 消费能力标识并取回其字符串所有权。
    ///
    /// # 参数
    ///
    /// - `value`：已经校验的 CapabilityId，消费后取回内部字符串。
    ///
    /// # 返回值
    ///
    /// 原始能力名称的 String。
    fn from(value: CapabilityId) -> Self {
        value.0
    }
}
impl fmt::Display for CapabilityId {
    /// 向格式化目标写入 `CapabilityId` 的显示文本。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `CapabilityId` 的共享引用。
    /// - `f`：接收格式化文本的写入目标。
    ///
    /// # 返回值
    ///
    /// 写入成功返回 `Ok(())`；格式化目标拒绝写入时返回 `fmt::Error`。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
