//! Worker 只接收授权、执行业务并回报, 不持有 ledger.

use crate::{
    capabilities,
    delivery::{Call, Inbox, WorkerReport},
};
use anyhow::{Context, Result};
use beamlet_primitives::Report;
use std::future::Future;
use tracing::{Instrument, info, info_span, warn};

/// Ctrl+C 后停止新交付并排空已接收队列, 不替 Authority 关闭 Operation.
pub(crate) async fn run(
    inbox: impl Inbox,
    session_id: String,
    max_concurrent_tasks: usize,
    shutdown: impl Future<Output = std::io::Result<()>>,
) -> Result<()> {
    run_with(inbox, session_id, max_concurrent_tasks, shutdown, execute).await
}

async fn run_with<F: Future<Output = Result<WorkerReport>> + Send + 'static>(
    mut inbox: impl Inbox,
    session_id: String,
    max_concurrent_tasks: usize,
    shutdown: impl Future<Output = std::io::Result<()>>,
    execute: impl Fn(Call, String) -> F + Clone + Send + 'static,
) -> Result<()> {
    tokio::pin!(shutdown);
    let mut stopping = false;
    let mut input_closed = false;
    let mut next_id = 0_u64;
    let mut tasks = tokio::task::JoinSet::new();

    info!(
        event = "role.ready",
        max_concurrent_tasks, "Worker 已就绪; 并发上限 0 表示无限"
    );

    while !input_closed || !tasks.is_empty() {
        let incoming = tokio::select! {
            // 满载时也及时停止新接收; 优先回收任务避免无限模式积压已完成句柄.
            biased;
            result = &mut shutdown, if !stopping => {
                result?;
                stopping = true;
                inbox.stop().await?;
                info!(event = "role.stopping", "停止接收新调用, 等待执行中任务和队列排空");
                continue;
            }
            completed = tasks.join_next(), if !tasks.is_empty() => {
                completed.context("缺少 Worker 执行任务")?.context("Worker 执行任务异常退出")??;
                continue;
            }
            incoming = inbox.receive(), if !input_closed && (max_concurrent_tasks == 0 || tasks.len() < max_concurrent_tasks) => incoming?,
        };
        let Some(incoming) = incoming else {
            input_closed = true;
            continue;
        };

        // 接收顺序分配身份, 与执行/回报完成顺序无关.
        next_id = next_id.checked_add(1).context("报告身份已耗尽")?;
        let report_id = format!("{session_id}:report:{next_id}");
        let execute = execute.clone();
        let span = info_span!(
            "attempt",
            operation_id = %incoming.call.attempt.operation_id(),
            attempt_id = %incoming.call.attempt.id(),
            capability = %incoming.call.capability,
            pool = %incoming.call.pool,
            worker_role_id = %incoming.call.worker.role_id,
            worker_session_id = %incoming.call.worker.session_id,
        );

        tasks.spawn(async move {
            info!(event = "operation.received", "收到 Operation 请求数据");
            info!(event = "attempt.received", "收到 Attempt");
            let result = execute(incoming.call, report_id).await;
            match result {
                Ok(report) => {
                    let span = info_span!("report", report_id = %report.id());
                    async {
                        info!(event = "report.send_started", "开始发送 Report");
                        match incoming.reply.send(report).await {
                            Ok(()) => info!(event = "report.sent", "Report 已提交至传输层"),
                            // 原 Authority 已离线不应让这个 Worker 停止处理其他调用.
                            Err(error) => warn!(event = "report.send_failed", error = %error, "发送 Report 失败"),
                        }
                    }.instrument(span).await;
                }
                Err(error) => warn!(event = "attempt.rejected", error = %error, "拒绝执行 Attempt"),
            }
            Ok::<_, anyhow::Error>(())
        }.instrument(span));
    }

    info!(event = "worker.drained", "已接收调用处理完毕");
    Ok(())
}

pub(crate) async fn execute(call: Call, report_id: String) -> Result<WorkerReport> {
    let result = capabilities::execute(call.capability, call.text).await?;

    let report = Report::new(report_id, call.attempt, result);
    info!(
        event = "report.created",
        operation_id = %report.operation_id(),
        attempt_id = %report.attempt_id(),
        report_id = %report.id(),
        "已创建 Report"
    );
    Ok(report)
}

#[cfg(test)]
mod concurrency_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use beamlet_primitives::Attempt;

    #[tokio::test]
    async fn rejects_unsupported_capability() {
        let unsupported = Call {
            pool: beamlet_api::Pool::default(),
            authority: beamlet_api::RoleInstance {
                role_id: uuid::Uuid::from_u128(1),
                session_id: uuid::Uuid::from_u128(2),
            },
            worker: beamlet_api::RoleInstance {
                role_id: uuid::Uuid::from_u128(3),
                session_id: uuid::Uuid::from_u128(4),
            },
            attempt: Attempt::from_parts("o".into(), "a".into()),
            capability: "text.reverse/v2".into(),
            text: "abc".into(),
        };
        assert!(execute(unsupported, "r".into()).await.is_err());
    }
}
