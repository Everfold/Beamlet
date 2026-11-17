//! v3 文本 profile 与业务数据的显式转换, 原语字段仅在此展开.
use crate::{Command, Outcome, Reply, ReportData, Submission, Task, WorkerCall};
use beamlet_api as api;
use beamlet_primitives::{Attempt, Report};

impl From<api::Task> for Task {
    /// 将 `api::Task` 按字段转换为 `Task`, 保留业务与路由关联.
    ///
    /// # 参数
    ///
    /// - `task`: 待转换的消息; 消费其字段所有权, 不执行序列化或版本校验.
    ///
    /// # 返回值
    ///
    /// 拥有转换后字段的 `Task`; 不进行网络发送或领域状态推进.
    fn from(task: api::Task) -> Self {
        Self {
            capability: task.capability,
            text: task.payload,
        }
    }
}
impl From<Task> for api::Task {
    /// 将 `Task` 按字段转换为 `api::Task`, 保留业务与路由关联.
    ///
    /// # 参数
    ///
    /// - `task`: 待转换的消息; 消费其字段所有权, 不执行序列化或版本校验.
    ///
    /// # 返回值
    ///
    /// 拥有转换后字段的 `api::Task`; 不进行网络发送或领域状态推进.
    fn from(task: Task) -> Self {
        Self {
            capability: task.capability,
            payload: task.text,
        }
    }
}
impl From<api::Submission> for Submission {
    /// 将 `api::Submission` 按字段转换为 `Submission`, 保留业务与路由关联.
    ///
    /// # 参数
    ///
    /// - `submission`: 待转换的消息; 消费其字段所有权, 不执行序列化或版本校验.
    ///
    /// # 返回值
    ///
    /// 拥有转换后字段的 `Submission`; 不进行网络发送或领域状态推进.
    fn from(submission: api::Submission) -> Self {
        Self {
            pool: submission.pool,
            worker: submission.worker,
            task: submission.task.into(),
        }
    }
}
impl From<Submission> for api::Submission {
    /// 将 `Submission` 按字段转换为 `api::Submission`, 保留业务与路由关联.
    ///
    /// # 参数
    ///
    /// - `submission`: 待转换的消息; 消费其字段所有权, 不执行序列化或版本校验.
    ///
    /// # 返回值
    ///
    /// 拥有转换后字段的 `api::Submission`; 不进行网络发送或领域状态推进.
    fn from(submission: Submission) -> Self {
        Self {
            pool: submission.pool,
            worker: submission.worker,
            task: submission.task.into(),
        }
    }
}
impl From<api::Command> for Command {
    /// 将 `api::Command` 按字段转换为 `Command`, 保留业务与路由关联.
    ///
    /// # 参数
    ///
    /// - `command`: 待转换的消息; 消费其字段所有权, 不执行序列化或版本校验.
    ///
    /// # 返回值
    ///
    /// 拥有转换后字段的 `Command`; 不进行网络发送或领域状态推进.
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
    /// 将 `Command` 按字段转换为 `api::Command`, 保留业务与路由关联.
    ///
    /// # 参数
    ///
    /// - `command`: 待转换的消息; 消费其字段所有权, 不执行序列化或版本校验.
    ///
    /// # 返回值
    ///
    /// 拥有转换后字段的 `api::Command`; 不进行网络发送或领域状态推进.
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
    /// 将 `api::Outcome` 按字段转换为 `Outcome`, 保留业务与路由关联.
    ///
    /// # 参数
    ///
    /// - `outcome`: 待转换的消息; 消费其字段所有权, 不执行序列化或版本校验.
    ///
    /// # 返回值
    ///
    /// 拥有转换后字段的 `Outcome`; 不进行网络发送或领域状态推进.
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
    /// 将 `Outcome` 按字段转换为 `api::Outcome`, 保留业务与路由关联.
    ///
    /// # 参数
    ///
    /// - `outcome`: 待转换的消息; 消费其字段所有权, 不执行序列化或版本校验.
    ///
    /// # 返回值
    ///
    /// 拥有转换后字段的 `api::Outcome`; 不进行网络发送或领域状态推进.
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
    /// 将 `api::Reply` 按字段转换为 `Reply`, 保留业务与路由关联.
    ///
    /// # 参数
    ///
    /// - `reply`: 待转换的消息; 消费其字段所有权, 不执行序列化或版本校验.
    ///
    /// # 返回值
    ///
    /// 拥有转换后字段的 `Reply`; 不进行网络发送或领域状态推进.
    fn from(reply: api::Reply) -> Self {
        Self {
            request_id: reply.request_id,
            authority: reply.authority,
            outcome: reply.outcome.into(),
        }
    }
}
impl From<Reply> for api::Reply {
    /// 将 `Reply` 按字段转换为 `api::Reply`, 保留业务与路由关联.
    ///
    /// # 参数
    ///
    /// - `reply`: 待转换的消息; 消费其字段所有权, 不执行序列化或版本校验.
    ///
    /// # 返回值
    ///
    /// 拥有转换后字段的 `api::Reply`; 不进行网络发送或领域状态推进.
    fn from(reply: Reply) -> Self {
        Self {
            request_id: reply.request_id,
            authority: reply.authority,
            outcome: reply.outcome.into(),
        }
    }
}

impl From<api::Call> for WorkerCall {
    /// 将 `api::Call` 按字段转换为 `WorkerCall`, 保留业务与路由关联.
    ///
    /// # 参数
    ///
    /// - `call`: 待转换的消息; 消费其字段所有权, 不执行序列化或版本校验.
    ///
    /// # 返回值
    ///
    /// 拥有转换后字段的 `WorkerCall`; 不进行网络发送或领域状态推进.
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
    /// 将 `WorkerCall` 按字段转换为 `api::Call`, 保留业务与路由关联.
    ///
    /// # 参数
    ///
    /// - `wire`: 待转换的消息; 消费其字段所有权, 不执行序列化或版本校验.
    ///
    /// # 返回值
    ///
    /// 拥有转换后字段的 `api::Call`; 不进行网络发送或领域状态推进.
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
    /// 将 `api::WorkerReport` 按字段转换为 `ReportData`, 保留业务与路由关联.
    ///
    /// # 参数
    ///
    /// - `report`: 待转换的消息; 消费其字段所有权, 不执行序列化或版本校验.
    ///
    /// # 返回值
    ///
    /// 拥有转换后字段的 `ReportData`; 不进行网络发送或领域状态推进.
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
    /// 将 `ReportData` 按字段转换为 `api::WorkerReport`, 保留业务与路由关联.
    ///
    /// # 参数
    ///
    /// - `wire`: 待转换的消息; 消费其字段所有权, 不执行序列化或版本校验.
    ///
    /// # 返回值
    ///
    /// 拥有转换后字段的 `api::WorkerReport`; 不进行网络发送或领域状态推进.
    fn from(wire: ReportData) -> Self {
        Report::new(
            wire.report_id,
            Attempt::from_parts(wire.operation_id, wire.attempt_id),
            wire.result,
        )
    }
}
