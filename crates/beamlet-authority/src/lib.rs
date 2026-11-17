//! Authority 的登记, 选择, 签发, 报告接受和控制处理, 不绑定传输或配置来源.
//!
//! 使用调用方的 Tokio 执行器. 输入需要 Clone 以同时保留原始请求和交付数据;
//! 结果只需 Send, 不要求 Clone, Debug 或序列化.
/// Authority 的公开编排入口, 只读 Directory 与可选 LocalInbox.
mod authority;
/// Authority 独立控制处理及本地/远端业务输入合并.
mod control;
/// Authority 的台账推进, 任务准入, 交付等待与结果回复.
mod engine;
/// 本库运行参数, 默认值及构造时的语义校验.
mod options;
/// Authority 对 Worker 完整公告的登记, 租约失效与实例选择.
mod registry;

pub use authority::{Authority, Directory, LocalInbox, Request, RunError};
pub use options::{BuildError, Concurrency, Options};

/// 通过可控输入与交付驱动 Authority 编排行为的单元测试.
#[cfg(test)]
#[path = "../tests/unit/ingress/mod.rs"]
mod ingress;
/// Authority 单元测试使用的模拟报告构造辅助.
#[cfg(test)]
#[path = "../tests/unit/support.rs"]
mod test_support;
