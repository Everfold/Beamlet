//! 外部请求入口与回复, 隐藏终端、消息主题、编码和回复地址.

#[cfg(test)]
pub(crate) mod channel;
mod contract;
pub(crate) mod live;
#[cfg(test)]
mod tests;

pub(crate) use contract::{Ingress, Reply, Request};
