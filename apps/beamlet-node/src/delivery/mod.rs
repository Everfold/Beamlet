//! 按能力单选交付, 调用者不接触主题、队列组或编码细节.
#[cfg(test)]
mod channel;
mod contract;
pub(crate) mod nats;
mod protocol;
#[cfg(test)]
mod tests;

pub(crate) use contract::{Call, Delivery, DeliveryError, Inbox, Incoming, WorkerReport};

/// 示例最多同时等待 16 次交付, 避免无界创建异步任务.
pub(crate) const MAX_IN_FLIGHT: usize = 16;
/// 小于 console 的五秒等待时间, 让它先收到明确的节点诊断.
pub(crate) const REPORT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);
/// 能力版本同时约定输入、输出及按 Unicode 标量值倒写的行为.
pub(crate) const REVERSE: &str = "text.reverse/v1";
