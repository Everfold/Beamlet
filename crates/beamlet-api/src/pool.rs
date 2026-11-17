use std::{fmt, str::FromStr};

/// 大小写敏感, 可安全用作单个主题片段的 pool 名称.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "String", into = "String"))]
pub struct Pool(
    /// 已校验的 pool 名称; 不包含会拆分主题片段的字符.
    String,
);

/// pool 名称为空, 过长或包含主题分隔符等无效字符.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("pool must contain 1..=64 ASCII letters, digits, '-' or '_'")]
pub struct InvalidPool;

impl TryFrom<String> for Pool {
    /// pool 名称不满足长度或字符约束时返回的校验错误.
    type Error = InvalidPool;

    /// 校验 pool 名称并构造可安全用于主题片段的标识.
    ///
    /// # 参数
    ///
    /// - `value`: pool 名称, 要求 1..=64 字节 ASCII 字母, 数字, 短横线或下划线.
    ///
    /// # 返回值
    ///
    /// 成功返回保留大小写的 Pool; 长度不在 1..=64 字节或字符非法时返回 InvalidPool.
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
    /// pool 名称不满足长度或字符约束时返回的校验错误.
    type Err = InvalidPool;
    /// 校验 pool 名称并构造可安全用于主题片段的标识.
    ///
    /// # 参数
    ///
    /// - `value`: pool 名称, 要求 1..=64 字节 ASCII 字母, 数字, 短横线或下划线.
    ///
    /// # 返回值
    ///
    /// 成功返回保留大小写的 Pool; 长度不在 1..=64 字节或字符非法时返回 InvalidPool.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_from(value.to_owned())
    }
}

impl From<Pool> for String {
    /// 消费 Pool 并取回名称所有权.
    ///
    /// # 参数
    ///
    /// - `value`: 已校验的 Pool, 消费后取回内部字符串.
    ///
    /// # 返回值
    ///
    /// 原始 pool 名称的 String.
    fn from(value: Pool) -> Self {
        value.0
    }
}

impl fmt::Display for Pool {
    /// 向格式化目标写入 `Pool` 的显示文本.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Pool` 的共享引用.
    /// - `f`: 接收格式化文本的写入目标.
    ///
    /// # 返回值
    ///
    /// 写入成功返回 `Ok(())`; 格式化目标拒绝写入时返回 `fmt::Error`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Default for Pool {
    /// 构造默认 pool 标识.
    ///
    /// # 返回值
    ///
    /// 名称精确为 `default` 的 Pool.
    fn default() -> Self {
        Self("default".into())
    }
}
