use std::fmt;

use uuid::Uuid;

/// 标识一个独立的执行授权.
///
/// UUID 由调用者提供, 不编码执行位置.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AttemptId(Uuid);

impl AttemptId {
    /// 返回此执行授权标识的 UUID 引用.
    #[must_use]
    pub const fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl From<Uuid> for AttemptId {
    /// 使用调用者提供的 UUID 构造执行授权标识.
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl From<AttemptId> for Uuid {
    /// 消耗执行授权标识并返回其 UUID.
    fn from(value: AttemptId) -> Self {
        value.0
    }
}

impl fmt::Display for AttemptId {
    /// 使用 UUID 的标准文本格式输出执行授权标识.
    ///
    /// # 错误
    ///
    /// 底层格式化写入失败时返回错误.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, formatter)
    }
}
