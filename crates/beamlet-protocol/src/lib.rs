//! Console 与节点共用的 v3 消息与主题, 不持有连接或管理 Operation 生命周期.

mod message;
pub mod subject;

pub use message::{
    Command, Discovery, DiscoveryReply, Outcome, Reply, ReportData, Submission, Task, WorkerCall,
    WorkerReply,
};

use serde::{Deserialize, Serialize, de::DeserializeOwned};

/// 线上消息的结构版本, 与业务能力版本独立.
pub const VERSION: u16 = 3;
/// 消息编码、解码或版本校验失败.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// JSON 消息无效.
    #[error("invalid protocol message: {0}")]
    Json(#[from] serde_json::Error),
    /// 不接受其他版本的消息.
    #[error("unsupported protocol version: {0}")]
    Version(u16),
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope<T> {
    version: u16,
    body: T,
}

/// 编码带显式版本的消息.
///
/// # Errors
/// 数据无法序列化为 JSON 时返回错误.
pub fn encode<T: Serialize>(message: &T) -> Result<Vec<u8>, Error> {
    Ok(serde_json::to_vec(&Envelope {
        version: VERSION,
        body: message,
    })?)
}

/// 解码消息并拒绝不支持的版本.
///
/// # Errors
/// 版本不匹配或结构无效时返回错误.
pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, Error> {
    let envelope: Envelope<serde_json::Value> = serde_json::from_slice(bytes)?;
    if envelope.version != VERSION {
        return Err(Error::Version(envelope.version));
    }
    Ok(serde_json::from_value(envelope.body)?)
}
