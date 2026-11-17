use std::{fmt, str::FromStr};

/// 带版本的能力名称, 长度为 1..=256 字节, 不自动修改大小写或空白.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "String", into = "String"))]
pub struct CapabilityId(String);

/// 能力标识为空或超过允许的字节长度.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("capability id must contain 1..=256 bytes")]
pub struct InvalidCapabilityId;

impl CapabilityId {
    /// 借用原始能力标识, 保留调用者提供的版本和大小写.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// 校验标识字节长度, 不分配内存或检查本地是否安装了该能力.
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
    type Error = InvalidCapabilityId;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::validate(&value)?;
        Ok(Self(value))
    }
}
impl FromStr for CapabilityId {
    type Err = InvalidCapabilityId;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::validate(value)?;
        Ok(Self(value.to_owned()))
    }
}
impl From<CapabilityId> for String {
    fn from(value: CapabilityId) -> Self {
        value.0
    }
}
impl fmt::Display for CapabilityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
