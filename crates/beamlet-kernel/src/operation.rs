use std::fmt;

use crate::{Attempt, ReportAssociation};

/// Operation 的生命周期状态, A 为外层定义的 Attempt 身份类型.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationState<A> {
    /// 尚未结束, 当前没有等待最终 Report 的 Attempt.
    Ready,
    /// 正在等待指定 Attempt 的最终 Report.
    AwaitingReport(A),
    /// Operation 已结束.
    Closed,
}

impl<A> OperationState<A> {
    /// 创建 Attempt, 返回该 Attempt 和等待报告的新状态, 原状态不变.
    ///
    /// 外层提供当前 Operation 身份和新的 Attempt 身份, 并保存两个返回值.
    /// 成功时克隆 Attempt 身份供状态记录, Operation 身份无需克隆.
    ///
    /// 已有等待报告的 Attempt 或 Operation 已结束时拒绝创建, 错误保留传入的身份.
    pub fn create_attempt<O>(
        &self,
        operation_id: O,
        attempt_id: A,
    ) -> Result<(Attempt<O, A>, Self), CreateAttemptError<O, A>>
    where
        A: Clone,
    {
        match self {
            Self::Ready => {
                let state = Self::AwaitingReport(attempt_id.clone());
                let attempt = Attempt {
                    id: attempt_id,
                    operation_id,
                };
                Ok((attempt, state))
            }
            Self::AwaitingReport(_) => Err(CreateAttemptError::AttemptPending {
                operation_id,
                attempt_id,
            }),
            Self::Closed => Err(CreateAttemptError::OperationClosed {
                operation_id,
                attempt_id,
            }),
        }
    }

    /// 接受匹配的最终 Report, 返回 Ready, 不自动结束 Operation.
    ///
    /// 外层须保证 operation_id 与当前状态属于同一个 Operation, 并保存返回的新状态.
    /// 此函数不修改输入, 不检查报告结果内容.
    ///
    /// 依次校验当前状态, Operation 关联和 Attempt 关联, 返回首个错误.
    pub fn accept_final_report<O: Eq>(
        &self,
        operation_id: &O,
        report: &ReportAssociation<O, A>,
    ) -> Result<Self, AcceptReportError>
    where
        A: Eq,
    {
        let pending_attempt = match self {
            Self::Ready => return Err(AcceptReportError::NoPendingAttempt),
            Self::Closed => return Err(AcceptReportError::OperationClosed),
            Self::AwaitingReport(attempt_id) => attempt_id,
        };

        if &report.operation_id != operation_id {
            return Err(AcceptReportError::OperationMismatch);
        }
        // 旧 Attempt 的报告不能解除对当前 Attempt 的等待.
        if &report.attempt_id != pending_attempt {
            return Err(AcceptReportError::AttemptMismatch);
        }

        Ok(Self::Ready)
    }
}

/// 创建 Attempt 失败的原因, 包含本次传入的身份.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CreateAttemptError<O, A> {
    /// 已有 Attempt 正在等待最终 Report.
    AttemptPending {
        /// 本次传入的 Operation 身份.
        operation_id: O,
        /// 本次传入的 Attempt 身份.
        attempt_id: A,
    },
    /// Operation 已结束.
    OperationClosed {
        /// 本次传入的 Operation 身份.
        operation_id: O,
        /// 本次传入的 Attempt 身份.
        attempt_id: A,
    },
}

impl<O, A> CreateAttemptError<O, A> {
    /// 按 Operation, Attempt 的顺序取回传入的身份.
    #[must_use]
    pub fn into_ids(self) -> (O, A) {
        match self {
            Self::AttemptPending {
                operation_id,
                attempt_id,
            }
            | Self::OperationClosed {
                operation_id,
                attempt_id,
            } => (operation_id, attempt_id),
        }
    }
}

impl<O, A> fmt::Display for CreateAttemptError<O, A> {
    /// 输出错误原因.
    ///
    /// 底层格式化写入失败时返回错误.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::AttemptPending { .. } => "an attempt is already awaiting a final report",
            Self::OperationClosed { .. } => "operation is closed",
        };
        formatter.write_str(message)
    }
}

impl<O: fmt::Debug, A: fmt::Debug> std::error::Error for CreateAttemptError<O, A> {}

/// 接受最终 Report 失败的原因.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AcceptReportError {
    /// 当前没有等待最终 Report 的 Attempt.
    NoPendingAttempt,
    /// Operation 已结束.
    OperationClosed,
    /// Report 引用的 Operation 与当前 Operation 不一致.
    OperationMismatch,
    /// Report 引用的 Attempt 与当前等待的 Attempt 不一致.
    AttemptMismatch,
}

impl fmt::Display for AcceptReportError {
    /// 输出错误原因.
    ///
    /// 底层格式化写入失败时返回错误.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::NoPendingAttempt => "no attempt is awaiting a final report",
            Self::OperationClosed => "operation is closed",
            Self::OperationMismatch => "report belongs to a different operation",
            Self::AttemptMismatch => "report belongs to a different attempt",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for AcceptReportError {}
