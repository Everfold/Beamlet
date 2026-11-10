//! 同进程角色之间的类型化交付与推进, 不选择运行时或创建基础设施

use crate::{
    Authority, LocalExecutionError, Worker,
    memory::{OperationPhase, StateError},
    model::{Delivery, DispatchAttempt, Message, SubmitReport},
};
use std::future::Future;

/// 尝试交付已接纳的原授权; 排队暂满时保留 Accepted, 由后续推进补送
///
/// 重复调用只补送尚未派发的原授权. 已缓存 Report 在同一调用中收录.
///
/// # 错误
/// 身份错配, 定义冲突或 Worker 记录表耗尽时返回错误; 不撤销 Authority 接纳事实
pub fn deliver<P, O, E>(
    authority: &mut Authority<P, O, E>,
    worker: &mut Worker<P, O, E>,
    dispatch: &Delivery<P, O, E>,
) -> Result<(), StateError>
where
    P: Clone + Eq + Send + 'static,
    O: Clone + Eq + Send + 'static,
    E: Clone + Eq + Send + 'static,
{
    let Message::DispatchAttempt(attempt) = &dispatch.message else {
        return Err(StateError::WrongMessage);
    };
    let operation = attempt.operation.operation_id;
    let view = authority
        .operation(operation)
        .ok_or(StateError::UnknownOperation)?;
    if view.dispatch != *dispatch {
        return Err(StateError::InvalidBinding);
    }
    if view.phase != OperationPhase::Accepted {
        return Ok(());
    }
    match worker.receive_dispatch(dispatch.clone()) {
        Ok(report) => {
            authority.mark_dispatched(operation)?;
            if let Some(report) = report {
                authority.receive_report(report)?;
            }
            Ok(())
        }
        Err(StateError::QueueFull) => Ok(()),
        Err(error) => Err(error),
    }
}

/// 补送本地待派发授权, 启动排队工作并收录下一份完成报告
///
/// 取消等待不会取消执行或遗失报告. 空闲返回 None, 宿主可将此等待与消息接收并列驱动.
/// 队列腾出空间后继续补送原授权, 不分配新 Operation 或 Attempt.
///
/// # 错误
/// 本地绑定, 记录容量或后台执行异常时返回错误, 不生成失败证据
///
/// # 恐慌
/// 启动工作时须处于 Tokio 运行时中; 执行函数构造过程恐慌时终止
pub async fn next_report<P, O, E, F, Fut>(
    authority: &mut Authority<P, O, E>,
    worker: &mut Worker<P, O, E>,
    execute: F,
) -> Result<Option<Delivery<P, O, E>>, LocalExecutionError>
where
    P: Clone + Eq + Send + 'static,
    O: Clone + Eq + Send + 'static,
    E: Clone + Eq + Send + 'static,
    F: Fn(DispatchAttempt<P>) -> Fut,
    Fut: Future<Output = SubmitReport<O, E>> + Send + 'static,
{
    loop {
        let pending: Vec<_> = authority
            .undispatched()
            .filter(|dispatch| worker.contains(dispatch.target_instance_id))
            .cloned()
            .collect();
        for dispatch in pending {
            deliver(authority, worker, &dispatch)?;
        }
        if worker.start_ready(&execute) == 0 {
            break;
        }
    }
    let Some(report) = worker.next_report(execute).await? else {
        return Ok(None);
    };
    authority.receive_report(report.clone())?;
    Ok(Some(report))
}
