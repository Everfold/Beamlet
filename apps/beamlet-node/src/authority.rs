//! Authority 管理生命周期, 通过交付接口将任务发给一个匹配的 Worker.
use crate::{
    delivery::{Call, Delivery, DeliveryError, MAX_IN_FLIGHT, REVERSE, WorkerReport},
    ingress::{Ingress, Reply, Request},
};
use anyhow::{Context, Result, ensure};
use beamlet_runtime::Runtime;
use std::{collections::HashMap, future::Future};
use tokio::task::JoinSet;

/// 入口与交付分别注入; 每份交付独立等待, 不阻塞其他输入和报告.
pub(crate) async fn run(
    mut ingress: impl Ingress,
    delivery: impl Delivery,
    instance: String,
    shutdown: impl Future<Output = std::io::Result<()>>,
) -> Result<()> {
    let mut stopping = false;
    let mut runtime = Runtime::new();
    let mut replies: HashMap<String, Reply> = HashMap::new();
    let mut deliveries: JoinSet<(String, Result<WorkerReport, DeliveryError>)> = JoinSet::new();
    let mut next_id = 0_u64;
    let mut unresolved = 0_usize;
    tokio::pin!(shutdown);
    println!("[Authority] 已就绪, 实例={instance}, 能力={REVERSE}; 输入文本后回车");

    loop {
        if stopping && deliveries.is_empty() {
            break;
        }
        let incoming = tokio::select! {
            result = &mut shutdown, if !stopping => {
                result?;
                stopping = true;
                // 停止接收新请求, 只等待已经签发 Attempt 的调用.
                ingress.stop().await?;
                println!("[Authority] 停止接收新请求, 等待在途交付结束");
                None
            }
            completed = deliveries.join_next(), if !deliveries.is_empty() => {
                let (id, outcome) = completed.context("缺少交付任务")?.context("交付任务异常退出")?;
                let result = match finish(&mut runtime, &id, outcome) {
                    Ok(text) => text,
                    Err(error) => {
                        unresolved += 1;
                        eprintln!("[Authority] 操作={id}, {error}; 保持等待报告, 不重试");
                        format!("错误: {error}; 操作保持等待报告, 未重试")
                    }
                };
                let reply = replies.remove(&id).context("缺少回复来源")?;
                reply.send(&id, result).await?;
                None
            }
            request = ingress.receive(), if !stopping && deliveries.len() < MAX_IN_FLIGHT => {
                let request = request?;
                if request.is_none() {
                    stopping = true;
                    ingress.stop().await?;
                }
                request
            }
        };
        if let Some(Request { text, reply }) = incoming {
            next_id = next_id.checked_add(1).context("操作身份已耗尽")?;
            let id = format!("{instance}:operation:{next_id}");
            runtime.create_operation(id.clone(), text.clone())?;
            let attempt = runtime.create_attempt(&id, format!("{instance}:attempt:{next_id}"))?;
            let call = Call {
                attempt,
                capability: REVERSE.into(),
                text,
            };
            replies.insert(id.clone(), reply);
            let delivery = delivery.clone();
            println!("[Authority] 已签发倒写授权, 操作={id}");
            deliveries.spawn(async move {
                let outcome = delivery.deliver(call).await;
                (id, outcome)
            });
        }
    }
    ingress.flush().await?;
    println!("[Authority] 已退出, 未取得有效报告的操作={unresolved}; 内存状态不持久化");
    Ok(())
}

/// 只有关联正确的 Report 能推进生命周期; 交付错误不伪装成报告.
fn finish(
    runtime: &mut Runtime<String, String, String>,
    id: &str,
    outcome: Result<WorkerReport, DeliveryError>,
) -> Result<String> {
    let report = outcome?;
    ensure!(
        report.operation_id() == id,
        "交付返回了其他 Operation 的报告"
    );
    runtime.accept_report(&report)?;
    runtime.close(report.operation_id())?;
    Ok(report.into_result())
}

#[cfg(test)]
mod tests {
    use super::*;
    use beamlet_primitives::{Attempt, Report};

    /// 所有交付错误及错误关联均保留当前 Attempt, 正确报告之后仍可接受.
    #[test]
    fn errors_preserve_pending_attempt() {
        let id = "operation".to_owned();
        let mut runtime = Runtime::new();
        runtime
            .create_operation(id.clone(), "hello".into())
            .unwrap();
        let attempt = runtime.create_attempt(&id, "attempt".into()).unwrap();
        for error in [
            DeliveryError::NoReceiver,
            DeliveryError::NotDelivered("full".into()),
            DeliveryError::Unknown("timeout".into()),
        ] {
            assert!(finish(&mut runtime, &id, Err(error)).is_err());
            assert_eq!(
                runtime
                    .operation(&id)
                    .unwrap()
                    .pending_attempt_id()
                    .map(String::as_str),
                Some("attempt")
            );
        }
        for (operation, attempt) in [("other", "attempt"), ("operation", "stale")] {
            let report = Report::new(
                "bad".into(),
                Attempt::from_parts(operation.into(), attempt.into()),
                "bad".into(),
            );
            assert!(finish(&mut runtime, &id, Ok(report)).is_err());
            assert_eq!(
                runtime
                    .operation(&id)
                    .unwrap()
                    .pending_attempt_id()
                    .map(String::as_str),
                Some("attempt")
            );
        }
        let report = Report::new("good".into(), attempt, "olleh".into());
        assert_eq!(finish(&mut runtime, &id, Ok(report)).unwrap(), "olleh");
        assert!(runtime.create_attempt(&id, "new".into()).is_err());
        let duplicate = Report::new(
            "good".into(),
            Attempt::from_parts(id.clone(), "attempt".into()),
            "olleh".into(),
        );
        assert!(finish(&mut runtime, &id, Ok(duplicate)).is_err());
    }
}
