use beamlet_kernel::{
    AcceptReportError, Attempt, CloseOperationError, CreateAttemptError, OperationState,
    ReportAssociation,
};

use crate::Operation;

/// 持有一个 Operation 及其当前状态, 数据只在本记录中更新.
#[derive(Debug, PartialEq, Eq)]
pub struct OperationRecord<O, A, P> {
    operation: Operation<O, P>,
    state: OperationState<A>,
}

impl<O, A, P> OperationRecord<O, A, P> {
    /// 接收一个新 Operation, 将生命周期初始化为 Ready.
    ///
    /// 同一 Operation 的后续转换应继续使用此记录.
    #[must_use]
    pub fn new(operation: Operation<O, P>) -> Self {
        Self {
            operation,
            state: OperationState::Ready,
        }
    }

    /// 借用绑定的不可变请求.
    pub fn operation(&self) -> &Operation<O, P> {
        &self.operation
    }

    /// 借用当前状态.
    pub fn state(&self) -> &OperationState<A> {
        &self.state
    }

    /// 创建 Attempt 并保存等待状态, 返回的 Attempt 由调用者保存.
    ///
    /// 调用者为每次新 Attempt 提供未使用的身份.
    /// 克隆 Operation 身份, 使返回的 Attempt 可以独立于记录持有.
    ///
    /// # Errors
    ///
    /// 等待报告或已经关闭时拒绝创建, 原状态不变, 错误保留传入的 Attempt 身份.
    pub fn create_attempt(
        &mut self,
        attempt_id: A,
    ) -> Result<Attempt<O, A>, CreateAttemptError<O, A>>
    where
        O: Clone,
        A: Clone,
    {
        let (attempt, state) = self
            .state
            .create_attempt(self.operation.id().clone(), attempt_id)?;
        self.state = state;
        Ok(attempt)
    }

    /// 校验报告关联并保存 Ready, 请求内容保持不变.
    ///
    /// 调用者保证报告对应已结束的 Attempt, 并保存完整报告及结果.
    /// 是否继续尝试或结束 Operation 由调用者决定.
    ///
    /// # Errors
    ///
    /// 状态不允许接受报告或关联不匹配时拒绝, 原状态不变.
    pub fn accept_report(
        &mut self,
        report: &ReportAssociation<O, A>,
    ) -> Result<(), AcceptReportError>
    where
        O: Eq,
        A: Eq,
    {
        self.state = self.state.accept_report(self.operation.id(), report)?;
        Ok(())
    }

    /// 保存 Closed, 已关闭时允许重复调用.
    ///
    /// # Errors
    ///
    /// 仍有 Attempt 等待报告时拒绝, 原状态不变.
    pub fn close(&mut self) -> Result<(), CloseOperationError> {
        self.state = self.state.close()?;
        Ok(())
    }
}
