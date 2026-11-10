//! Worker 公告和心跳的严格原型 Adapter, 不维护观察状态
use super::protocol::ProtocolError;
use beamlet::worker_observation;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
/// 单条生命周期消息的字节上限
pub const MAX_BYTES: usize = 4096;

fn invalid_sequence() -> ProtocolError {
    ProtocolError::Json(serde::de::Error::custom(
        "Worker 心跳序号无效或启动公告携带了序号",
    ))
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Kind {
    WorkerStarted,
    WorkerHeartbeat,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
/// Worker 自报归属及心跳的严格 v0 JSON Adapter
pub struct Announcement {
    protocol_version: u8,
    kind: Kind,
    /// Node 稳定身份
    pub node_id: Uuid,
    /// Node 本次运行
    pub node_instance_id: Uuid,
    /// Worker 稳定身份
    pub worker_id: Uuid,
    /// Worker 本次运行
    pub worker_instance_id: Uuid,
    /// 目标 Authority 本次运行
    pub authority_instance_id: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sequence: Option<u64>,
}

impl Announcement {
    /// 构造同一实例的启动公告, 不声明在线
    pub fn started(
        node_id: Uuid,
        node_instance_id: Uuid,
        worker_id: Uuid,
        worker_instance_id: Uuid,
        authority_instance_id: Uuid,
    ) -> Self {
        Self {
            protocol_version: 0,
            kind: Kind::WorkerStarted,
            node_id,
            node_instance_id,
            worker_id,
            worker_instance_id,
            authority_instance_id,
            sequence: None,
        }
    }

    /// 构造原归属的新心跳; 编码及投影时校验非零序号
    pub fn heartbeat(&self, sequence: u64) -> Self {
        Self {
            kind: Kind::WorkerHeartbeat,
            sequence: Some(sequence),
            ..self.clone()
        }
    }

    /// 校验并投影类型化输入, 不包含路由或认证判断
    /// # 错误
    /// 版本, 种类或序号无效时拒绝
    pub fn observation(&self) -> Result<worker_observation::Announcement, ProtocolError> {
        self.validate()?;
        Ok(worker_observation::Announcement {
            binding: worker_observation::Binding {
                node_id: self.node_id.as_u128().into(),
                node_instance_id: self.node_instance_id.as_u128().into(),
                worker_id: self.worker_id.as_u128().into(),
                worker_instance_id: self.worker_instance_id.as_u128().into(),
                authority_instance_id: self.authority_instance_id.as_u128().into(),
            },
            signal: match self.kind {
                Kind::WorkerStarted => worker_observation::Signal::Started,
                Kind::WorkerHeartbeat => worker_observation::Signal::Heartbeat(
                    self.sequence
                        .and_then(std::num::NonZeroU64::new)
                        .ok_or_else(invalid_sequence)?,
                ),
            },
        })
    }

    fn validate(&self) -> Result<(), ProtocolError> {
        if self.protocol_version != 0 {
            return Err(ProtocolError::UnsupportedVersion(
                self.protocol_version.into(),
            ));
        }
        if !match self.kind {
            Kind::WorkerStarted => self.sequence.is_none(),
            Kind::WorkerHeartbeat => self.sequence.is_some_and(|sequence| sequence > 0),
        } {
            return Err(invalid_sequence());
        }
        Ok(())
    }

    /// 严格解码有界 Worker 公告
    /// # 错误
    /// 超限, 版本或结构无效时拒绝
    pub fn from_json(bytes: &[u8]) -> Result<Self, ProtocolError> {
        if bytes.len() > MAX_BYTES {
            return Err(ProtocolError::MessageTooLarge);
        }
        let announcement: Self = serde_json::from_slice(bytes)?;
        announcement.validate()?;
        Ok(announcement)
    }

    /// 编码原公告并检查上限
    /// # 错误
    /// 序号, 版本或编码无效时拒绝
    pub fn to_json(&self) -> Result<Vec<u8>, ProtocolError> {
        self.validate()?;
        let bytes = serde_json::to_vec(self)?;
        if bytes.len() > MAX_BYTES {
            return Err(ProtocolError::MessageTooLarge);
        }
        Ok(bytes)
    }

    /// 按完整实例绑定构造发送地址
    pub fn topic(&self) -> String {
        destination(self.authority_instance_id, self.worker_instance_id)
    }
}

/// Authority 独立的 Worker 生命周期订阅地址
pub fn inbox(authority: Uuid) -> String {
    format!(
        "beamlet/v0/authority/{}/lifecycle/worker/*",
        authority.simple()
    )
}

fn destination(authority: Uuid, worker: Uuid) -> String {
    format!(
        "beamlet/v0/authority/{}/lifecycle/worker/{}",
        authority.simple(),
        worker.simple()
    )
}
