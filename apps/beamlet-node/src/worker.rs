//! Worker 只接收授权、执行业务并回报, 不持有 runtime.

use crate::{
    delivery::{Call, Inbox, WorkerReport},
    text_reverse::{self, CAPABILITY},
};
use anyhow::{Context, Result};
use beamlet_primitives::Report;
use std::future::Future;

/// Ctrl+C 后停止新交付并排空已接收队列, 不替 Authority 关闭 Operation.
pub(crate) async fn run(
    mut inbox: impl Inbox,
    instance: String,
    shutdown: impl Future<Output = std::io::Result<()>>,
) -> Result<()> {
    tokio::pin!(shutdown);
    let mut stopping = false;
    let mut next_id = 0_u64;

    println!("[Worker节点] 已就绪, 实例={instance}, 能力={CAPABILITY}");

    loop {
        let incoming = tokio::select! {
            result = &mut shutdown, if !stopping => {
                result?;
                stopping = true;
                inbox.stop().await?;
                println!("[Worker节点] 停止接收新调用, 排空已接收队列");
                continue;
            }
            incoming = inbox.receive() => incoming?,
        };
        let Some(incoming) = incoming else { break };

        next_id = next_id.checked_add(1).context("报告身份已耗尽")?;
        let report_id = format!("{instance}:report:{next_id}");

        println!(
            "[Worker节点] 收到调用, 操作={}, 授权={}",
            incoming.call.attempt.operation_id(),
            incoming.call.attempt.id()
        );

        match execute(incoming.call, report_id) {
            Ok(report) => {
                if let Err(error) = incoming.reply.send(report).await {
                    // 原 Authority 已离线不应让这个 Worker 停止处理其他调用.
                    eprintln!("[Worker节点] 返回报告失败: {error}");
                }
            }
            Err(error) => eprintln!("[Worker节点] 拒绝执行: {error}"),
        }
    }

    println!("[Worker节点] 已退出");

    Ok(())
}

pub(crate) fn execute(call: Call, report_id: String) -> Result<WorkerReport> {
    let result = text_reverse::execute(&call.capability, &call.text)?;

    Ok(Report::new(report_id, call.attempt, result))
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
