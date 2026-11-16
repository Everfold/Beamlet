//! 按 pool、能力与实例单选交付, 调用者不接触主题或编码细节.

#[cfg(test)]
pub(crate) mod channel;
mod contract;
pub(crate) mod nats;
mod protocol;
#[cfg(test)]
mod tests;

pub(crate) use contract::{Call, Delivery, DeliveryError, Inbox, Incoming, WorkerReport};

/// 示例最多同时等待 16 次交付, 避免无界创建异步任务.
pub(crate) const MAX_IN_FLIGHT: usize = 16;

pub(crate) use beamlet_protocol::REPORT_TIMEOUT;
