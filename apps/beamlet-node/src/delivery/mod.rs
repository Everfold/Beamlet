//! 按 pool、能力与实例单选交付, 调用者不接触主题或编码细节.

#[cfg(test)]
pub(crate) mod channel;
mod contract;
pub(crate) mod nats;
mod protocol;
#[cfg(test)]
mod tests;

pub(crate) use contract::{Call, Delivery, DeliveryError, Inbox, Incoming, WorkerReport};
