//! Authority 管理生命周期, 通过交付接口将任务发给一个匹配的 Worker.
use crate::delivery::{Call, Delivery, DeliveryError, MAX_IN_FLIGHT, REVERSE, WorkerReport};
use anyhow::{Context, Result, ensure};
use beamlet_runtime::Runtime;
use futures::StreamExt;
use std::{collections::HashMap, io::BufRead};
use tokio::{sync::mpsc, task::JoinSet};

/// 外部输入的回复地址不进入 Operation、Attempt 或 Report.
enum Destination {
    Local,
    Nats(async_nats::Subject),
}

/// NATS 与本地输入共用 runtime; 每份交付独立等待, 不阻塞其他输入和报告.
pub(crate) async fn run(
    client: async_nats::Client,
    delivery: impl Delivery,
    instance: String,
) -> Result<()> {
    let mut subscription = client
        .queue_subscribe("beamlet.reverse", "beamlet-nodes".into())
        .await?;
    client.flush().await?;
    let mut input = terminal_input();
    let mut input_closed = false;
    let mut stopping = false;
    let mut runtime = Runtime::new();
    let mut destinations = HashMap::new();
    let mut deliveries: JoinSet<(String, Result<WorkerReport, DeliveryError>)> = JoinSet::new();
    let mut next_id = 0_u64;
    let mut unresolved = 0_usize;
    let signal = tokio::signal::ctrl_c();
    tokio::pin!(signal);
    println!("[Authority] 已就绪, 实例={instance}, 能力={REVERSE}; 输入文本后回车");

    loop {
        if stopping && deliveries.is_empty() {
            break;
        }
        let incoming = tokio::select! {
            result = &mut signal, if !stopping => {
                result?;
                stopping = true;
                // 停止接收新请求, 只等待已经签发 Attempt 的调用.
                subscription.unsubscribe().await?;
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
                let destination = destinations.remove(&id).context("缺少回复来源")?;
                match destination {
                    Destination::Local => println!("[Authority] 本地结果: {result}"),
                    Destination::Nats(subject) => {
                        client.publish(subject, result.into()).await?;
                        println!("[Authority] 已回复远端请求, 操作={id}");
                    }
                }
                None
            }
            line = input.recv(), if !input_closed && !stopping && deliveries.len() < MAX_IN_FLIGHT => {
                match line {
                    Some(line) => Some((line.context("读取本地输入失败")?, Destination::Local)),
                    None => { input_closed = true; None }
                }
            }
            message = subscription.next(), if !stopping && deliveries.len() < MAX_IN_FLIGHT => {
                let message = message.context("NATS 订阅已关闭")?;
                match message.reply {
                    Some(reply) => match String::from_utf8(message.payload.to_vec()) {
                        Ok(text) => Some((text, Destination::Nats(reply))),
                        Err(_) => {
                            client.publish(reply, "错误: 消息必须是 UTF-8 文本".into()).await?;
                            None
                        }
                    },
                    None => {
                        eprintln!("[Authority] 忽略没有回复地址的消息");
                        None
                    }
                }
            }
        };
        if let Some((text, destination)) = incoming {
            next_id = next_id.checked_add(1).context("操作身份已耗尽")?;
            let id = format!("{instance}:operation:{next_id}");
            runtime.create_operation(id.clone(), text.clone())?;
            let attempt = runtime.create_attempt(&id, format!("{instance}:attempt:{next_id}"))?;
            let call = Call {
                attempt,
                capability: REVERSE.into(),
                text,
            };
            destinations.insert(id.clone(), destination);
            let delivery = delivery.clone();
            println!("[Authority] 已签发倒写授权, 操作={id}");
            deliveries.spawn(async move {
                let outcome = delivery.deliver(call).await;
                (id, outcome)
            });
        }
    }
    client.flush().await?;
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

/// 阻塞终端读取放在独立线程, 本地 EOF 不影响 NATS 服务.
fn terminal_input() -> mpsc::Receiver<std::io::Result<String>> {
    let (send, recv) = mpsc::channel(MAX_IN_FLIGHT);
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            if send.blocking_send(line).is_err() {
                break;
            }
        }
    });
    recv
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
