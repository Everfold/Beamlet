use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

/// 大小写敏感、可安全用作单个主题片段的 pool 名称.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Pool(String);

/// pool 名称为空、过长或包含主题分隔符等无效字符.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("pool must contain 1..=64 ASCII letters, digits, '-' or '_'")]
pub struct InvalidPool;

impl TryFrom<String> for Pool {
    type Error = InvalidPool;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty()
            || value.len() > 64
            || !value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(InvalidPool);
        }
        Ok(Self(value))
    }
}

impl FromStr for Pool {
    type Err = InvalidPool;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_from(value.to_owned())
    }
}

impl From<Pool> for String {
    fn from(value: Pool) -> Self {
        value.0
    }
}

impl fmt::Display for Pool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Default for Pool {
    fn default() -> Self {
        Self("default".into())
    }
}
