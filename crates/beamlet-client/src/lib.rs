//! 可并发调用的 Authority 客户端, 负责发现, 关联校验和等待期限.
//!
//! 调用方提供传输和 Options, 使用调用方的 Tokio 执行器; 不读取配置, 不管理 UI
//! 选择, 不创建后台任务或隐含并发队列. 输入和结果无需 Clone, Debug 或 serde.
//! 超时或丢弃调用 future 只停止本地等待, 不取消远端任务, 不自动重试.
/// 消息后端无关的发现收集, 请求期限和回复关联校验.
mod client;
/// 本库运行参数, 默认值及构造时的语义校验.
mod options;
/// Client 命令校验, 预期回复约束及可判别调用错误.
mod request;

pub use client::Client;
pub use options::{BuildError, Options};
pub use request::Error;
