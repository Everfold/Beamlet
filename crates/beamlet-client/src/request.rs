use beamlet_api::{CapabilityId, Command, Outcome, Pool, RoleInstance, Selection, TransportError};

/// 调用失败, 与 Authority 返回的 Completed / Rejected / Unresolved 业务结果分开.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// 本地校验失败, 尚未发送请求.
    #[error("invalid request: {0}")]
    InvalidRequest(&'static str),
    /// 传输失败; 请求可能已执行, 不自动重试.
    #[error("request transport failed; execution may have started: {0}")]
    Transport(#[from] TransportError),
    /// 超过总等待期限; 不证明请求未执行.
    #[error("request timed out; execution may have started")]
    Timeout,
    /// 请求、角色会话或返回的数据与本次调用不匹配.
    #[error("reply does not match the request")]
    InvalidReply,
    /// Worker 查询被 Authority 明确拒绝; 提交拒绝保留为 Outcome::Rejected.
    #[error("worker query rejected: {reason}")]
    Rejected { reason: String },
}

pub(crate) fn valid_instance(instance: RoleInstance) -> bool {
    !instance.role_id.is_nil() && !instance.session_id.is_nil()
}

pub(crate) enum Expected {
    Workers {
        pool: Pool,
        capability: Option<String>,
    },
    Submit {
        worker: Selection,
    },
}
impl Expected {
    pub(crate) fn for_command<P>(command: &Command<P>) -> Result<Self, Error> {
        if command.request_id().is_nil() || !valid_instance(command.authority()) {
            return Err(Error::InvalidRequest("nil request or authority UUID"));
        }
        let capability = match command {
            Command::Workers { capability, .. } => capability.as_deref(),
            Command::Submit { submission, .. } => Some(submission.task.capability.as_str()),
        };
        if capability.is_some_and(|id| CapabilityId::validate(id).is_err()) {
            return Err(Error::InvalidRequest("invalid capability"));
        }
        Ok(match command {
            Command::Workers {
                pool, capability, ..
            } => Self::Workers {
                pool: pool.clone(),
                capability: capability.clone(),
            },
            Command::Submit { submission, .. } => {
                if let Selection::Exact(worker) = submission.worker
                    && !valid_instance(worker)
                {
                    return Err(Error::InvalidRequest("nil worker UUID"));
                }
                Self::Submit {
                    worker: submission.worker,
                }
            }
        })
    }
    pub(crate) fn check<R>(&self, outcome: &Outcome<R>) -> Result<(), Error> {
        let valid = match (self, outcome) {
            (_, Outcome::Rejected { .. }) => true,
            (Self::Workers { pool, capability }, Outcome::Workers { workers }) => {
                workers.iter().all(|view| {
                    &view.presence.pool == pool
                        && valid_instance(view.presence.instance)
                        && capability
                            .as_ref()
                            .is_none_or(|id| view.presence.capabilities.contains(id))
                })
            }
            (
                Self::Submit { worker: selected },
                Outcome::Completed {
                    operation_id,
                    attempt_id,
                    report_id,
                    worker,
                    ..
                },
            ) => {
                !operation_id.is_empty()
                    && !attempt_id.is_empty()
                    && !report_id.is_empty()
                    && matches_worker(*selected, *worker)
            }
            (
                Self::Submit { worker: selected },
                Outcome::Unresolved {
                    operation_id,
                    attempt_id,
                    worker,
                    ..
                },
            ) => {
                !operation_id.is_empty()
                    && !attempt_id.is_empty()
                    && matches_worker(*selected, *worker)
            }
            _ => false,
        };
        if valid {
            Ok(())
        } else {
            Err(Error::InvalidReply)
        }
    }
}
fn matches_worker(selected: Selection, actual: RoleInstance) -> bool {
    valid_instance(actual)
        && match selected {
            Selection::Any => true,
            Selection::Exact(expected) => actual == expected,
        }
}
