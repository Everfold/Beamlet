//! Authority 的登记、选择、签发、报告接受和控制处理, 不绑定传输或配置来源.
//!
//! 使用调用方的 Tokio 执行器. 输入需要 Clone 以同时保留原始请求和交付数据;
//! 结果只需 Send, 不要求 Clone、Debug 或序列化.
mod authority;
mod control;
mod engine;
mod options;
mod registry;

pub use authority::{Authority, Directory, LocalInbox, Request, RunError};
pub use options::{BuildError, Concurrency, Options};

#[cfg(test)]
mod ingress;
#[cfg(test)]
mod test_support;
