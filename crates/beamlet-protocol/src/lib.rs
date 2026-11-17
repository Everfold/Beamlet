//! Console 与节点共用的 v3 线格式, 不持有连接或管理 Operation 生命周期.

/// 泛型语义数据与 v3 文本 profile 之间的显式字段转换。
mod conversion;
/// v3 文本线格式的发现、请求、调用与报告消息。
mod message;

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
    Json(
        /// JSON 编码或解码的原始 serde_json 错误。
        #[from]
        serde_json::Error,
    ),
    /// 不接受其他版本的消息.
    #[error("unsupported protocol version: {0}")]
    Version(
        /// 报文携带但当前实现不支持的协议版本号。
        u16,
    ),
}

/// JSON 线协议的外层封装，将结构版本与消息体放在同一个报文中。
///
/// # 泛型参数
///
/// - `T`：协议消息体类型。
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope<T> {
    /// 报文结构版本，与业务能力版本独立。
    version: u16,
    /// 当前版本对应的具体消息体。
    body: T,
}

/// 把可序列化消息封装为带版本的 v3 JSON 字节。
///
/// # 参数
///
/// - `message`：需要编码的消息，通过 serde 序列化为带版本的 JSON。
///
/// # 返回值
///
/// 成功返回编码后的字节；序列化失败返回协议 Error。
///
/// # Errors
/// 数据无法序列化为 JSON 时返回错误.
pub fn encode<T: Serialize>(message: &T) -> Result<Vec<u8>, Error> {
    Ok(serde_json::to_vec(&Envelope {
        version: VERSION,
        body: message,
    })?)
}

/// 解析并校验 v3 JSON 封装，再还原指定消息类型。
///
/// # 参数
///
/// - `bytes`：待解码的完整 JSON 封装字节，包含协议版本和消息体。
///
/// # 返回值
///
/// 成功返回消息；版本不支持、格式或消息结构错误返回协议 Error。
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
