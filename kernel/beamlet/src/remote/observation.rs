use super::{Client, Transport, query_on};
use crate::{
    ReportObservation,
    observe::{Budget, Codec, Error, Notifications, State},
    query::{self, Observation, Selection},
};
use tokio::time::{Instant, sleep_until};

impl<T: Transport, C: Codec> Client<T, C>
where
    C::Task: Eq,
    C::Output: Clone + Eq,
    C::Failure: Clone + Eq,
{
    /// 合并已建立通道上的通知与有界查询, 返回本 State 尚未交付的已采用报告
    ///
    /// 通知通道须由应用在调用前建立, 查询回复使用独立队列. 每次调用立即对账,
    /// 等待期间按预算重复查询; 订阅传播未完成或通知丢失均可由查询补齐.
    /// allocate 为每次查询生成全新关联 ID. 不提交, 重投执行或重建连接.
    /// 取消或断线后保留 State, 可在新 Client/通知通道上继续; 同一 State 不重复交付.
    ///
    /// # 错误
    /// 预算/目标无效, 查询或通道失败, 合并冲突/容量不足, 总期限到达时返回错误
    pub async fn observe<N: Notifications, F: FnMut() -> query::Id>(
        &mut self,
        state: &mut State<C::Task, C::Output, C::Failure>,
        notices: &N,
        budget: Budget,
        mut allocate: F,
    ) -> Result<Option<ReportObservation<C::Output, C::Failure>>, Error<C::Error, T::Error, N::Error>>
    {
        if budget.timeout.is_zero()
            || budget.poll_interval.is_zero()
            || budget.query_timeout.is_zero()
            || budget.query_timeout > budget.timeout
        {
            return Err(Error::InvalidBudget);
        }
        if self.expected_authority != Some(state.authority()) {
            return Err(Error::WrongAuthority);
        }
        let deadline = Instant::now()
            .checked_add(budget.timeout)
            .ok_or(Error::InvalidBudget)?;
        let mut next_query = Instant::now();
        loop {
            if state.delivered() {
                return Ok(None);
            }
            if let Some(report) = state.take_adopted_report() {
                return Ok(Some(report));
            }
            let request = query::Request {
                id: allocate(),
                authority_instance_id: state.authority(),
                selection: state.missing_report().map_or(
                    Selection::Operation(state.operation_id()),
                    Selection::Report,
                ),
            };
            let query_start = next_query;
            let reconcile = async {
                sleep_until(query_start).await;
                let query_deadline = Instant::now()
                    .checked_add(budget.query_timeout)
                    .unwrap_or(deadline)
                    .min(deadline);
                query_on(&self.transport, &self.codec, request, query_deadline).await
            };
            tokio::pin!(reconcile);
            let result = loop {
                // Notifications can stay ready continuously, including filtered messages.
                tokio::task::yield_now().await;
                tokio::select! {
                    biased;
                    _ = sleep_until(deadline) => return Err(Error::TimedOut),
                    result = &mut reconcile => break result,
                    received = notices.recv() => {
                        let Some(bytes) = received.map_err(Error::Notifications)? else { continue; };
                        let Ok(notice) = self.codec.decode_notice(&bytes) else { continue; };
                        // A shared subscription can carry other operations; they are not conflicts.
                        if notice.operation.binding.authority_instance_id != state.authority()
                            || notice.operation.binding.operation_id != state.operation_id()
                            || !notice.is_valid() { continue; }
                        state.merge_notice(notice)?;
                        if let Some(report) = state.take_adopted_report() { return Ok(Some(report)); }
                    }
                }
            };
            let queried = result.is_ok();
            match result {
                Ok(Observation::Operation(view)) => {
                    state.merge_operation(view)?;
                }
                Ok(Observation::Report(report)) => {
                    state.merge_report(report)?;
                }
                Ok(Observation::NotFound) => unreachable!("query_on returns NotFound as an error"),
                Err(query::ClientError::TimedOut { .. }) => {}
                Err(error) => return Err(Error::Query(error)),
            }
            // Fetch a newly discovered report body immediately. Failed queries retain the interval.
            next_query = if queried && state.missing_report().is_some() {
                Instant::now()
            } else {
                Instant::now()
                    .checked_add(budget.poll_interval)
                    .unwrap_or(deadline)
                    .min(deadline)
            };
        }
    }
}
