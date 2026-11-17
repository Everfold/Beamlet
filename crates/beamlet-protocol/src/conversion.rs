//! v3 文本 profile 与业务数据的显式转换, 原语字段仅在此展开.
use crate::{Command, Outcome, Reply, ReportData, Submission, Task, WorkerCall};
use beamlet_api as api;
use beamlet_primitives::{Attempt, Report};

impl From<api::Task> for Task {
    fn from(task: api::Task) -> Self {
        Self {
            capability: task.capability,
            text: task.payload,
        }
    }
}
impl From<Task> for api::Task {
    fn from(task: Task) -> Self {
        Self {
            capability: task.capability,
            payload: task.text,
        }
    }
}
impl From<api::Submission> for Submission {
    fn from(submission: api::Submission) -> Self {
        Self {
            pool: submission.pool,
            worker: submission.worker,
            task: submission.task.into(),
        }
    }
}
impl From<Submission> for api::Submission {
    fn from(submission: Submission) -> Self {
        Self {
            pool: submission.pool,
            worker: submission.worker,
            task: submission.task.into(),
        }
    }
}
impl From<api::Command> for Command {
    fn from(command: api::Command) -> Self {
        match command {
            api::Command::Workers {
                request_id,
                authority,
                pool,
                capability,
            } => Self::Workers {
                request_id,
                authority,
                pool,
                capability,
            },
            api::Command::Submit {
                request_id,
                authority,
                submission,
            } => Self::Submit {
                request_id,
                authority,
                submission: submission.into(),
            },
        }
    }
}
impl From<Command> for api::Command {
    fn from(command: Command) -> Self {
        match command {
            Command::Workers {
                request_id,
                authority,
                pool,
                capability,
            } => Self::Workers {
                request_id,
                authority,
                pool,
                capability,
            },
            Command::Submit {
                request_id,
                authority,
                submission,
            } => Self::Submit {
                request_id,
                authority,
                submission: submission.into(),
            },
        }
    }
}
impl From<api::Outcome> for Outcome {
    fn from(outcome: api::Outcome) -> Self {
        match outcome {
            api::Outcome::Rejected { reason } => Self::Rejected { reason },
            api::Outcome::Workers { workers } => Self::Workers { workers },
            api::Outcome::Completed {
                operation_id,
                attempt_id,
                report_id,
                worker,
                result,
            } => Self::Completed {
                operation_id,
                attempt_id,
                report_id,
                worker,
                result,
            },
            api::Outcome::Unresolved {
                operation_id,
                attempt_id,
                worker,
                reason,
            } => Self::Unresolved {
                operation_id,
                attempt_id,
                worker,
                reason,
            },
        }
    }
}
impl From<Outcome> for api::Outcome {
    fn from(outcome: Outcome) -> Self {
        match outcome {
            Outcome::Rejected { reason } => Self::Rejected { reason },
            Outcome::Workers { workers } => Self::Workers { workers },
            Outcome::Completed {
                operation_id,
                attempt_id,
                report_id,
                worker,
                result,
            } => Self::Completed {
                operation_id,
                attempt_id,
                report_id,
                worker,
                result,
            },
            Outcome::Unresolved {
                operation_id,
                attempt_id,
                worker,
                reason,
            } => Self::Unresolved {
                operation_id,
                attempt_id,
                worker,
                reason,
            },
        }
    }
}
impl From<api::Reply> for Reply {
    fn from(reply: api::Reply) -> Self {
        Self {
            request_id: reply.request_id,
            authority: reply.authority,
            outcome: reply.outcome.into(),
        }
    }
}
impl From<Reply> for api::Reply {
    fn from(reply: Reply) -> Self {
        Self {
            request_id: reply.request_id,
            authority: reply.authority,
            outcome: reply.outcome.into(),
        }
    }
}

impl From<api::Call> for WorkerCall {
    fn from(call: api::Call) -> Self {
        let (operation_id, attempt_id) = call.attempt.into_parts();
        Self {
            pool: call.pool,
            authority: call.authority,
            worker: call.worker,
            operation_id,
            attempt_id,
            task: Task {
                capability: call.capability,
                text: call.payload,
            },
        }
    }
}
impl From<WorkerCall> for api::Call {
    fn from(wire: WorkerCall) -> Self {
        Self {
            pool: wire.pool,
            authority: wire.authority,
            worker: wire.worker,
            attempt: Attempt::from_parts(wire.operation_id, wire.attempt_id),
            capability: wire.task.capability,
            payload: wire.task.text,
        }
    }
}
impl From<api::WorkerReport> for ReportData {
    fn from(report: api::WorkerReport) -> Self {
        let (report_id, attempt, result) = report.into_parts();
        let (operation_id, attempt_id) = attempt.into_parts();
        Self {
            report_id,
            operation_id,
            attempt_id,
            result,
        }
    }
}
impl From<ReportData> for api::WorkerReport {
    fn from(wire: ReportData) -> Self {
        Report::new(
            wire.report_id,
            Attempt::from_parts(wire.operation_id, wire.attempt_id),
            wire.result,
        )
    }
}
