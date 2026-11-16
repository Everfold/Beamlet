//! Worker 只接收授权、执行业务并回报, 不持有 runtime.

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
    mut inbox: impl Inbox,
    session_id: String,
    shutdown: impl Future<Output = std::io::Result<()>>,
) -> Result<()> {
    tokio::pin!(shutdown);
    let mut stopping = false;
    let mut next_id = 0_u64;

    info!(event = "node.ready", "Worker 已就绪");

    loop {
        let incoming = tokio::select! {
            result = &mut shutdown, if !stopping => {
                result?;
                stopping = true;
                inbox.stop().await?;
                info!(event = "node.stopping", "停止接收新调用, 排空已接收队列");
                continue;
            }
            incoming = inbox.receive() => incoming?,
        };
        let Some(incoming) = incoming else { break };

        next_id = next_id.checked_add(1).context("报告身份已耗尽")?;
        let report_id = format!("{session_id}:report:{next_id}");

        let span = info_span!(
            "attempt",
            operation_id = %incoming.call.attempt.operation_id(),
            attempt_id = %incoming.call.attempt.id(),
            capability = %incoming.call.capability,
        );

        async {
            info!(event = "operation.received", "收到 Operation 请求数据");
            info!(event = "attempt.received", "收到 Attempt");
            let span = tracing::Span::current();
            let result = tokio::task::spawn_blocking(move || {
                let _entered = span.enter();
                execute(incoming.call, report_id)
            }).await.context("能力执行任务异常退出")?;
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
                    }
                    .instrument(span)
                    .await;
                }
                Err(error) => warn!(event = "attempt.rejected", error = %error, "拒绝执行 Attempt"),
            }
            Ok::<_, anyhow::Error>(())
        }
        .instrument(span)
        .await?;
    }

    info!(event = "worker.drained", "已接收调用处理完毕");

    Ok(())
}

pub(crate) fn execute(call: Call, report_id: String) -> Result<WorkerReport> {
    let result = capabilities::execute(&call.capability, &call.text)?;

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
mod tests {
    use super::*;
    use beamlet_primitives::Attempt;

    #[test]
    fn rejects_unsupported_capability() {
        let unsupported = Call {
            attempt: Attempt::from_parts("o".into(), "a".into()),
            capability: "text.reverse/v2".into(),
            text: "abc".into(),
        };
        assert!(execute(unsupported, "r".into()).is_err());
    }
}
