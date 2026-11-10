//! 生命周期消息的应用 I/O, 时钟与展示, 观察规则由 beamlet 提供
use anyhow::{Context, Result, ensure};
use beamlet::worker_observation::{Observations, Source, Transition};
use beamlet_agent::testing::worker_lifecycle::Announcement;
use beamlet_message_zenoh::Transport;
use std::{collections::HashMap, time::Duration};
use tokio::time::{MissedTickBehavior, interval, timeout};
use uuid::Uuid;
const ANNOUNCE_TIMEOUT: Duration = Duration::from_secs(10);
const HEARTBEAT_PERIOD: Duration = Duration::from_secs(1);
const HEARTBEAT_SEND_TIMEOUT: Duration = Duration::from_millis(500);
const OBSERVATION_PERIOD: Duration = Duration::from_millis(250);

pub(crate) fn record(
    observations: &mut Observations,
    announcement: Announcement,
    source: Source,
) -> Result<()> {
    let input = announcement.observation()?;
    let change = observations.record(input, source)?;
    let authority = announcement.authority_instance_id;
    let worker = announcement.worker_instance_id;
    if change.transition == Some(Transition::ExpiredThenOnline) {
        println!(
            "Authority 心跳观察: 实例={authority} WorkerInstanceId={worker} 状态=可疑; 心跳观察已超时, 不代表执行停止"
        );
    }
    if change.transition.is_some() {
        println!(
            "Authority 心跳观察: 实例={authority} WorkerInstanceId={worker} 状态=在线; 仅为近期消息观察, 不代表执行接纳"
        );
    }
    if change.binding_added {
        println!(
            "Authority 已记录 Worker 归属: 实例={authority} WorkerId={} WorkerInstanceId={worker} NodeId={} NodeInstanceId={}; 绑定不是身份认证或执行接纳",
            announcement.worker_id, announcement.node_id, announcement.node_instance_id
        );
    }
    Ok(())
}

pub(crate) async fn observe(
    mut observations: Observations,
    transport: Transport,
    local: Vec<Announcement>,
) -> Result<()> {
    let mut clock = interval(OBSERVATION_PERIOD);
    clock.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut local_clock = interval(HEARTBEAT_PERIOD);
    local_clock.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut sequence = 0_u64;
    loop {
        let received = tokio::select! {
            _ = clock.tick() => { while let Some(worker) = observations.poll_expired() {
                println!("Authority 心跳观察: 实例={} WorkerInstanceId={worker} 状态=可疑; 心跳观察已超时或尚未收到, 不代表执行停止", observations.authority());
            } continue; }
            _ = local_clock.tick(), if !local.is_empty() => {
                sequence = sequence.checked_add(1).context("本地 Worker 心跳序号已耗尽")?;
                for announcement in &local { record(&mut observations, announcement.heartbeat(sequence), Source::Local)?; }
                continue;
            }
            received = transport.recv_message() => received,
        };
        let message = match received {
            Ok(message) => message,
            Err(
                error @ (beamlet_message_zenoh::Error::MessageTooLarge
                | beamlet_message_zenoh::Error::UnexpectedDelete),
            ) => {
                eprintln!("已忽略 Worker 生命周期消息: {error}");
                continue;
            }
            Err(error) => return Err(error).context("接收 Worker 生命周期消息失败"),
        };
        let result = (|| {
            let announcement = Announcement::from_json(&message.payload)?;
            ensure!(
                message.key_expr == announcement.topic(),
                "Worker 生命周期消息路由与实例绑定不符"
            );
            record(&mut observations, announcement, Source::Remote)
        })();
        if let Err(error) = result {
            eprintln!("已忽略 Worker 生命周期消息: {error}");
        }
    }
}

async fn publish(
    transport: &Transport,
    announcement: &Announcement,
    budget: Duration,
) -> Result<()> {
    timeout(budget, async {
        let bytes = announcement.to_json()?;
        let sender = transport.sender(&announcement.topic()).await?;
        sender.wait_for_receiver().await?;
        sender.send(bytes).await?;
        Ok::<_, anyhow::Error>(())
    })
    .await
    .context("Worker 生命周期消息发送超时")?
}

async fn announce(transport: &Transport, announcement: &Announcement) {
    match publish(transport, announcement, ANNOUNCE_TIMEOUT).await {
        Ok(()) => println!(
            "Worker 启动公告已发布: 实例={}; 未确认 Authority 收录",
            announcement.worker_instance_id
        ),
        Err(error) => eprintln!(
            "Worker 启动公告未确认: 实例={}; {error}; 不自动重试, 不影响接收 Attempt",
            announcement.worker_instance_id
        ),
    }
}

async fn heartbeats(transport: &Transport, announcement: &Announcement) -> Result<()> {
    let mut clock = interval(HEARTBEAT_PERIOD);
    clock.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut sequence = 0_u64;
    let mut publication_failed = false;
    loop {
        clock.tick().await;
        sequence = sequence.checked_add(1).context("Worker 心跳序号已耗尽")?;
        match publish(
            transport,
            &announcement.heartbeat(sequence),
            HEARTBEAT_SEND_TIMEOUT,
        )
        .await
        {
            Ok(()) => {
                if publication_failed {
                    println!(
                        "Worker 心跳发布恢复: 实例={}; 未确认 Authority 收录",
                        announcement.worker_instance_id
                    );
                }
                publication_failed = false;
            }
            Err(error) => {
                if !publication_failed {
                    eprintln!(
                        "Worker 心跳发布未确认: 实例={}; {error}; 丢弃本次观察, 不影响 Attempt 或 Report",
                        announcement.worker_instance_id
                    );
                }
                publication_failed = true;
            }
        }
    }
}

pub(crate) async fn publish_starts(
    transport: &Transport,
    notices: Vec<beamlet_agent::testing::lifecycle::RoleNotice>,
) -> Result<()> {
    let messages = notices
        .into_iter()
        .map(|notice| Ok::<_, anyhow::Error>((notice.topic(), notice.to_json()?)))
        .collect::<Result<Vec<_>>>()?;
    let mut clock = interval(Duration::from_secs(2));
    clock.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut publication_failed = false;
    loop {
        clock.tick().await;
        let mut failed = false;
        for (topic, bytes) in &messages {
            // 公共公告不等待 Client 存在, 后续重播仍沿用同一启动事实
            let result = timeout(Duration::from_millis(250), async {
                transport.sender(topic).await?.send(bytes.clone()).await
            })
            .await;
            if !matches!(result, Ok(Ok(()))) {
                failed = true;
            }
        }
        if failed && !publication_failed {
            eprintln!("角色启动公告发布未确认; 不阻塞业务驱动");
        }
        if !failed && publication_failed {
            println!("角色启动公告发布恢复; 未确认 Client 收录");
        }
        publication_failed = failed;
    }
}

pub(crate) async fn publish_exits(
    transport: &Transport,
    notices: Vec<beamlet_agent::testing::lifecycle::RoleNotice>,
    outcomes: &HashMap<Uuid, beamlet_agent::testing::lifecycle::ExitOutcome>,
) {
    use beamlet_agent::testing::lifecycle::ExitOutcome;

    // 全 Node 共用两秒发布预算, 不等待 Client 匹配或收录, 不重试
    let result = timeout(Duration::from_secs(2), async {
        for notice in notices {
            let outcome = outcomes
                .get(&notice.role_instance_id)
                .copied()
                .unwrap_or(ExitOutcome::Unconfirmed);
            let notice = notice.exited(outcome);
            let result = timeout(Duration::from_millis(250), async {
                let bytes = notice.to_json()?;
                transport.sender(&notice.topic()).await?.send(bytes).await?;
                Ok::<_, anyhow::Error>(())
            })
            .await;
            match result {
                Ok(Ok(())) => println!(
                    "{} 退出通知已发布: 实例={}; 排空观察={outcome}; 未确认 Client 收录",
                    notice.role, notice.role_instance_id
                ),
                _ => eprintln!(
                    "{} 退出通知发布未确认: 实例={}; 不重试",
                    notice.role, notice.role_instance_id
                ),
            }
        }
    })
    .await;
    if result.is_err() {
        eprintln!("角色退出通知发布预算到期; 部分通知可能未送达, 不延长 Draining");
    }
}

pub(crate) async fn maintain(transport: &Transport, announcement: Announcement) -> Result<()> {
    tokio::try_join!(
        async {
            announce(transport, &announcement).await;
            Ok::<_, anyhow::Error>(())
        },
        heartbeats(transport, &announcement),
    )
    .map(|_| ())
}
