//! 固定 Hello 任务的严格查询与结果通知编码, 只读投影 Authority 事实

use super::{
    hello::{self, Response, ResponseMessage, Task},
    protocol::{Envelope, Limits, PROTOCOL_VERSION, ProtocolError},
    submission::{self, HelloClientCodec},
};
use beamlet::{
    Authority, ExecutionBinding, OperationObservation, ReportObservation,
    id::RoleInstanceId,
    model::Message,
    query::{self, Codec, Observation, Reply},
    state::{OperationPhase, WaitStatus},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 指定 Authority 的只读查询路由前缀, 后缀由应用的独立回复会话决定
pub fn prefix(authority: RoleInstanceId) -> String {
    format!(
        "beamlet/v0/authority/{}/query/",
        Uuid::from_u128(authority.as_u128()).simple()
    )
}

/// 解码并只读回答本实例查询, 不接收 Worker 或身份分配函数
///
/// # 错误
/// 编码, 上限, 请求目标或查询结果关联无效时返回错误, 不生成 NotFound 代替
pub fn respond(
    authority: &Authority<Task, String, String>,
    bytes: &[u8],
    limits: &Limits,
) -> Result<Vec<u8>, ProtocolError> {
    let wire: Request = submission::decode(bytes, limits)?;
    submission::check(wire.protocol_version)?;
    let reply = authority
        .answer_query(wire.into_core())
        .map_err(|_| invalid("查询目标不是当前 Authority"))?;
    let observation = ObservationWire::from_core(reply.observation)?;
    submission::encode(
        &ReplyWire {
            request: Request::from_core(reply.request),
            observation,
        },
        limits,
    )
}

impl Codec for HelloClientCodec {
    type Task = Task;
    type Output = String;
    type Failure = String;
    type Error = ProtocolError;

    fn encode_query(&self, request: query::Request) -> Result<Vec<u8>, ProtocolError> {
        submission::encode(&Request::from_core(request), &self.limits)
    }

    fn decode_query(&self, bytes: &[u8]) -> Result<Reply<Task, String, String>, ProtocolError> {
        let wire: ReplyWire = submission::decode(bytes, &self.limits)?;
        submission::check(wire.request.protocol_version)?;
        let request = wire.request.into_core();
        let observation = wire.observation.into_core(&self.limits)?;
        let reply = Reply {
            request,
            observation,
        };
        if !reply.matches(request) {
            return Err(invalid("查询回复选择或执行绑定不一致"));
        }
        Ok(reply)
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Request {
    protocol_version: u16,
    request_id: Uuid,
    authority_instance_id: Uuid,
    selection: Selection,
}

impl Request {
    fn from_core(request: query::Request) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            request_id: Uuid::from_u128(request.id.as_u128()),
            authority_instance_id: Uuid::from_u128(request.authority_instance_id.as_u128()),
            selection: match request.selection {
                query::Selection::Submission(id) => Selection::Submission {
                    id: Uuid::from_u128(id.as_u128()),
                },
                query::Selection::Operation(id) => Selection::Operation {
                    id: Uuid::from_u128(id.as_u128()),
                },
                query::Selection::Report(id) => Selection::Report {
                    id: Uuid::from_u128(id.as_u128()),
                },
            },
        }
    }
    fn into_core(self) -> query::Request {
        query::Request {
            id: self.request_id.as_u128().into(),
            authority_instance_id: self.authority_instance_id.as_u128().into(),
            selection: match self.selection {
                Selection::Submission { id } => query::Selection::Submission(id.as_u128().into()),
                Selection::Operation { id } => query::Selection::Operation(id.as_u128().into()),
                Selection::Report { id } => query::Selection::Report(id.as_u128().into()),
            },
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Selection {
    Submission { id: Uuid },
    Operation { id: Uuid },
    Report { id: Uuid },
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ReplyWire {
    request: Request,
    observation: ObservationWire,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum ObservationWire {
    Operation {
        binding: Binding,
        task: Task,
        phase: Phase,
        wait_status: Wait,
        adopted_report: Option<Uuid>,
    },
    Report {
        envelope: Envelope,
    },
    NotFound {},
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Binding {
    authority: Uuid,
    worker: Uuid,
    operation: Uuid,
    attempt: Uuid,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
    Accepted,
    Dispatched,
    Reported,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Wait {
    Waiting,
    TimedOut,
}

fn invalid(message: &str) -> ProtocolError {
    ProtocolError::Json(serde::de::Error::custom(message))
}

impl ObservationWire {
    fn from_core(observation: Observation<Task, String, String>) -> Result<Self, ProtocolError> {
        Ok(match observation {
            Observation::Operation(view) => ObservationWire::Operation {
                binding: Binding {
                    authority: Uuid::from_u128(view.binding.authority_instance_id.as_u128()),
                    worker: Uuid::from_u128(view.binding.worker_instance_id.as_u128()),
                    operation: Uuid::from_u128(view.binding.operation_id.as_u128()),
                    attempt: Uuid::from_u128(view.binding.attempt_id.as_u128()),
                },
                task: view.task,
                phase: match view.phase {
                    OperationPhase::Accepted => Phase::Accepted,
                    OperationPhase::Dispatched => Phase::Dispatched,
                    OperationPhase::Reported => Phase::Reported,
                    _ => return Err(invalid("不支持的 Operation 观察阶段")),
                },
                wait_status: match view.wait_status {
                    WaitStatus::Waiting => Wait::Waiting,
                    WaitStatus::TimedOut => Wait::TimedOut,
                    _ => return Err(invalid("不支持的等待观察")),
                },
                adopted_report: view.adopted_report.map(|id| Uuid::from_u128(id.as_u128())),
            },
            Observation::Report(view) => {
                let ResponseMessage::Report(envelope) = Response::report(
                    &view.report,
                    view.worker_instance_id,
                    view.authority_instance_id,
                )
                .into_message() else {
                    unreachable!("Report 包装只能返回证据");
                };
                ObservationWire::Report { envelope }
            }
            Observation::NotFound => ObservationWire::NotFound {},
        })
    }
    fn into_core(
        self,
        limits: &Limits,
    ) -> Result<Observation<Task, String, String>, ProtocolError> {
        Ok(match self {
            ObservationWire::Operation {
                binding,
                task,
                phase,
                wait_status,
                adopted_report,
            } => Observation::Operation(OperationObservation {
                binding: ExecutionBinding {
                    authority_instance_id: binding.authority.as_u128().into(),
                    worker_instance_id: binding.worker.as_u128().into(),
                    operation_id: binding.operation.as_u128().into(),
                    attempt_id: binding.attempt.as_u128().into(),
                },
                task,
                phase: match phase {
                    Phase::Accepted => OperationPhase::Accepted,
                    Phase::Dispatched => OperationPhase::Dispatched,
                    Phase::Reported => OperationPhase::Reported,
                },
                wait_status: match wait_status {
                    Wait::Waiting => WaitStatus::Waiting,
                    Wait::TimedOut => WaitStatus::TimedOut,
                },
                adopted_report: adopted_report.map(|id| id.as_u128().into()),
            }),
            ObservationWire::Report { envelope } => {
                envelope.validate(limits)?;
                let delivery = hello::report_from_envelope(envelope)?;
                let Message::SubmitReport(report) = delivery.message else {
                    unreachable!("证据解码只能返回 Report")
                };
                Observation::Report(ReportObservation::new(
                    delivery.target_instance_id,
                    delivery.source_instance_id,
                    report,
                ))
            }
            ObservationWire::NotFound {} => Observation::NotFound,
        })
    }
}

/// 一个 Authority 的共享结果通知路由, 由应用显式订阅
pub fn notice_topic(authority: RoleInstanceId) -> String {
    format!(
        "beamlet/v0/authority/{}/observations",
        Uuid::from_u128(authority.as_u128()).simple()
    )
}

/// 编码 Authority 已保留的执行观察, 不声明订阅可靠性
/// # 错误
/// 非法通知, 格式或字节上限不满足时返回错误
pub fn encode_notice(
    notice: beamlet::observe::Notice<Task, String, String>,
    limits: &Limits,
) -> Result<Vec<u8>, ProtocolError> {
    if !notice.is_valid() {
        return Err(invalid("通知的执行绑定或证据无效"));
    }
    submission::encode(
        &NoticeWire {
            protocol_version: PROTOCOL_VERSION,
            operation: ObservationWire::from_core(Observation::Operation(notice.operation))?,
            report: ObservationWire::from_core(Observation::Report(notice.report))?,
        },
        limits,
    )
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct NoticeWire {
    protocol_version: u16,
    operation: ObservationWire,
    report: ObservationWire,
}

impl beamlet::observe::Codec for HelloClientCodec {
    fn decode_notice(&self, bytes: &[u8]) -> beamlet::observe::DecodeResult<Self> {
        let wire: NoticeWire = submission::decode(bytes, &self.limits)?;
        submission::check(wire.protocol_version)?;
        let (Observation::Operation(operation), Observation::Report(report)) = (
            wire.operation.into_core(&self.limits)?,
            wire.report.into_core(&self.limits)?,
        ) else {
            return Err(invalid("通知必须包含执行快照和完整报告"));
        };
        let notice = beamlet::observe::Notice { operation, report };
        if !notice.is_valid() {
            return Err(invalid("通知的执行绑定或证据无效"));
        }
        Ok(notice)
    }
}
