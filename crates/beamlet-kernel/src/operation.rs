use std::fmt;

use crate::ReportAssociation;

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
    /// # 错误
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
