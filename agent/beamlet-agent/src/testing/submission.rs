//! Client 提交及接纳关联协议, 与执行授权的 Operation 编码分开
use super::{
    hello::{Response, ResponseMessage, Task},
    protocol::{Limits, Message, PROTOCOL_VERSION, ProtocolError},
};
use beamlet::model::{Delivery, Message as DomainMessage};
use beamlet::{
    ReportObservation,
    id::SubmissionKey,
    remote::{Binding, Codec, Outcome, Reply},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 固定 Hello 协议与通用远端 Client 之间的编码 Adapter
#[derive(Debug, Default)]
pub struct HelloClientCodec {
    pub(super) limits: Limits,
}

impl HelloClientCodec {
    /// 使用应用提供的协议上限, 校验在编码和解码入口执行
    pub fn new(limits: Limits) -> Self {
        Self { limits }
    }
}

impl Codec for HelloClientCodec {
    type Task = Task;
    type Output = String;
    type Failure = String;
    type Error = ProtocolError;

    fn encode(&self, key: SubmissionKey, task: &Task) -> Result<Vec<u8>, ProtocolError> {
        Submission {
            protocol_version: PROTOCOL_VERSION,
            submission_key: Uuid::from_u128(key.as_u128()),
            task: *task,
        }
        .to_json(&self.limits)
    }

    fn decode(&self, bytes: &[u8]) -> Result<Reply<String, String>, ProtocolError> {
        let reply = SubmissionReply::from_json(bytes, &self.limits)?;
        let key = reply.submission_key.as_u128().into();
        let authority_instance_id = reply.authority_instance_id.as_u128().into();
        if let Some(reason) = reply.error {
            return Ok(Reply::Rejected {
                key,
                authority_instance_id,
                reason,
            });
        }
        let binding = Binding {
            authority_instance_id,
            operation_id: reply.operation_id.ok_or_else(invalid)?.as_u128().into(),
            attempt_id: reply.attempt_id.ok_or_else(invalid)?.as_u128().into(),
        };
        let outcome = reply
            .response
            .map(|response| -> Result<_, ProtocolError> {
                match response.into_message() {
                    ResponseMessage::Report(envelope) => {
                        let delivery = super::hello::report_from_envelope(envelope)?;
                        let DomainMessage::SubmitReport(report) = delivery.message else {
                            unreachable!("Report 转换必须返回执行证据");
                        };
                        Ok(Outcome::Report(ReportObservation::new(
                            delivery.target_instance_id,
                            delivery.source_instance_id,
                            report,
                        )))
                    }
                    ResponseMessage::Pending { .. } => Ok(Outcome::Pending),
                    ResponseMessage::SubmissionError { error, .. } => {
                        Ok(Outcome::SubmissionError(error))
                    }
                }
            })
            .transpose()?;
        Ok(Reply::Accepted {
            key,
            binding,
            outcome,
        })
    }
}

/// Client 重投时必须保持同一 key 和完整定义
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Submission {
    protocol_version: u16,
    pub submission_key: Uuid,
    pub task: Task,
}
impl Submission {
    /// 构造固定任务提交, 不分配 OperationId
    pub fn hello(submission_key: Uuid) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            submission_key,
            task: Task::Hello,
        }
    }
    /// 有界编码
    /// # 错误
    /// JSON 或协议校验失败时返回错误
    pub fn to_json(&self, limits: &Limits) -> Result<Vec<u8>, ProtocolError> {
        check(self.protocol_version)?;
        encode(self, limits)
    }
    /// 严格有界解码
    /// # 错误
    /// JSON, 版本或大小不合法时返回错误
    pub fn from_json(bytes: &[u8], limits: &Limits) -> Result<Self, ProtocolError> {
        let value: Self = decode(bytes, limits)?;
        check(value.protocol_version)?;
        Ok(value)
    }
}
/// 接纳映射随每一份结果返回, 允许 Report 先于接纳回执到达
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SubmissionReply {
    protocol_version: u16,
    pub submission_key: Uuid,
    pub authority_instance_id: Uuid,
    pub operation_id: Option<Uuid>,
    pub attempt_id: Option<Uuid>,
    pub response: Option<Response>,
    pub error: Option<String>,
}
impl SubmissionReply {
    /// 将库返回的原授权投影为接纳映射, 不代表已执行
    ///
    /// # 错误
    /// 领域消息不是 DispatchAttempt 时返回错误
    pub fn accepted<P, O, E>(
        key: Uuid,
        delivery: &Delivery<P, O, E>,
    ) -> Result<Self, ProtocolError> {
        let DomainMessage::DispatchAttempt(attempt) = &delivery.message else {
            return Err(invalid());
        };
        Ok(Self {
            protocol_version: PROTOCOL_VERSION,
            submission_key: key,
            authority_instance_id: Uuid::from_u128(delivery.source_instance_id.as_u128()),
            operation_id: Some(Uuid::from_u128(attempt.operation.operation_id.as_u128())),
            attempt_id: Some(Uuid::from_u128(attempt.attempt_id.as_u128())),
            response: None,
            error: None,
        })
    }
    /// 未接纳的提交错误, 不创建业务身份或 Report
    pub fn rejected(key: Uuid, authority: Uuid, error: String) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            submission_key: key,
            authority_instance_id: authority,
            operation_id: None,
            attempt_id: None,
            response: None,
            error: Some(error),
        }
    }
    /// 在原接纳映射中携带后续观察
    pub fn with_response(&self, response: Response) -> Self {
        Self {
            response: Some(response),
            ..self.clone()
        }
    }
    /// 编码并校验完整身份关联
    /// # 错误
    /// 结构或关联非法时返回错误
    pub fn to_json(&self, limits: &Limits) -> Result<Vec<u8>, ProtocolError> {
        self.validate(limits)?;
        encode(self, limits)
    }
    /// 严格解码并校验完整身份关联
    /// # 错误
    /// 大小, JSON 或关联非法时返回错误
    pub fn from_json(bytes: &[u8], limits: &Limits) -> Result<Self, ProtocolError> {
        let value: Self = decode(bytes, limits)?;
        value.validate(limits)?;
        Ok(value)
    }
    fn validate(&self, limits: &Limits) -> Result<(), ProtocolError> {
        check(self.protocol_version)?;
        if self.error.is_some() {
            if self.operation_id.is_some() || self.attempt_id.is_some() || self.response.is_some() {
                return Err(invalid());
            }
            return Ok(());
        }
        let (Some(op), Some(attempt)) = (self.operation_id, self.attempt_id) else {
            return Err(invalid());
        };
        if let Some(response) = &self.response {
            response.to_json(limits)?;
            match response.message() {
                ResponseMessage::Report(envelope) => {
                    let Message::SubmitReport(report) = &envelope.message else {
                        return Err(invalid());
                    };
                    if report.operation_id.as_u128() != op.as_u128()
                        || report.attempt_id.as_u128() != attempt.as_u128()
                        || envelope.target_instance_id.as_u128()
                            != self.authority_instance_id.as_u128()
                    {
                        return Err(invalid());
                    }
                }
                ResponseMessage::Pending { operation_id }
                | ResponseMessage::SubmissionError { operation_id, .. } => {
                    if *operation_id != op {
                        return Err(invalid());
                    }
                }
            }
        }
        Ok(())
    }
}
fn invalid() -> ProtocolError {
    ProtocolError::Json(serde::de::Error::custom("提交回复身份关联或结构不合法"))
}
pub(super) fn check(version: u16) -> Result<(), ProtocolError> {
    if version != PROTOCOL_VERSION {
        return Err(ProtocolError::UnsupportedVersion(version));
    }
    Ok(())
}
pub(super) fn encode<T: Serialize>(value: &T, limits: &Limits) -> Result<Vec<u8>, ProtocolError> {
    limits.validate()?;
    let bytes = serde_json::to_vec(value)?;
    if bytes.len() > limits.max_message_bytes {
        return Err(ProtocolError::MessageTooLarge);
    }
    Ok(bytes)
}
pub(super) fn decode<T: serde::de::DeserializeOwned>(
    bytes: &[u8],
    limits: &Limits,
) -> Result<T, ProtocolError> {
    limits.validate()?;
    if bytes.len() > limits.max_message_bytes {
        return Err(ProtocolError::MessageTooLarge);
    }
    Ok(serde_json::from_slice(bytes)?)
}
