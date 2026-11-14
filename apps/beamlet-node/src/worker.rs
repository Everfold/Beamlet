//! 内存 channel 直接传递授权和报告, 不再编码为 JSON.
use anyhow::{Context, Result};
use beamlet_primitives::{Attempt, Report};
use tokio::sync::mpsc;

pub type Call = (Attempt<u64, u64>, String);
pub type WorkerReport = Report<u64, u64, u64, String>;

/// 接收授权和文本, 返回完整 Report; 请求 channel 关闭后处理完队列再退出.
pub async fn run(
    mut calls: mpsc::Receiver<Call>,
    reports: mpsc::Sender<WorkerReport>,
) -> Result<()> {
    let mut report_id = 0_u64;

    while let Some((attempt, text)) = calls.recv().await {
        report_id = report_id.checked_add(1).context("报告身份已耗尽")?;
        let operation_id = *attempt.operation_id();

        // 倒写按 Unicode 标量值处理. 此轻量计算无需独立线程.
        let result: String = text.chars().rev().collect();
        let report = Report::new(report_id, attempt, result);
        reports.send(report).await.context("报告接收端已关闭")?;

        println!("[工作任务] 已发送报告={report_id}, 操作={operation_id}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 关闭请求端后仍处理全部队列, 保留每份授权身份及不同文本的结果.
    #[tokio::test]
    async fn drains_calls_and_preserves_identity() {
        let (send, calls) = mpsc::channel(2);
        let (reports, mut receive) = mpsc::channel(1);
        let worker = tokio::spawn(run(calls, reports));
        send.send((Attempt::from_parts(7, 41), "Hello 世界".into()))
            .await
            .unwrap();
        send.send((Attempt::from_parts(9, 53), "".into()))
            .await
            .unwrap();
        drop(send);
        for (operation, attempt, id, result) in [(7, 41, 1, "界世 olleH"), (9, 53, 2, "")] {
            let report = receive.recv().await.unwrap();
            assert_eq!(*report.operation_id(), operation);
            assert_eq!(*report.attempt_id(), attempt);
            assert_eq!(*report.id(), id);
            assert_eq!(report.result(), result);
        }
        assert!(receive.recv().await.is_none());
        worker.await.unwrap().unwrap();
    }

    /// Authority 已退出时返回错误, 不继续处理没有接收方的工作.
    #[tokio::test]
    async fn closed_report_receiver_stops_worker() {
        let (send, calls) = mpsc::channel(1);
        let (reports, receive) = mpsc::channel(1);
        drop(receive);
        send.send((Attempt::from_parts(1, 1), "abc".into()))
            .await
            .unwrap();
        assert!(run(calls, reports).await.is_err());
    }
}
