use std::fmt;

use uuid::Uuid;

/// 标识一个独立的执行报告.
///
/// UUID 由调用者提供, 不编码执行位置.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReportId(Uuid);

impl ReportId {
    /// 返回此执行报告标识的 UUID 引用.
    #[must_use]
    pub const fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl From<Uuid> for ReportId {
    /// 使用调用者提供的 UUID 构造执行报告标识.
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl From<ReportId> for Uuid {
    /// 消耗执行报告标识并返回其 UUID.
    fn from(value: ReportId) -> Self {
        value.0
    }
}

impl fmt::Display for ReportId {
    /// 使用 UUID 的标准文本格式输出执行报告标识.
    ///
    /// # 错误
    ///
    /// 底层格式化写入失败时返回错误.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, formatter)
    }
}
