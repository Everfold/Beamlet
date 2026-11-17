//! 按 pool、能力与实例单选交付, 调用者不接触主题或编码细节.

#[cfg(test)]
pub(crate) mod channel;
mod contract;
pub(crate) mod nats;
#[cfg(test)]
mod tests;

pub(crate) use beamlet_api::{Call, WorkerReport};
pub(crate) use contract::{Delivery, DeliveryError, Inbox, Incoming};
