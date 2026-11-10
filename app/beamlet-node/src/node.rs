//! Node 组合入口, 网络会话与静态业务角色分别配置

use std::{collections::HashMap, sync::Arc};

use anyhow::{Context, Result, ensure};
use beamlet::{
    Worker, composition,
    id::{NodeInstanceId, RoleInstanceId},
    lifecycle::{Completion, DEFAULT_DRAIN_TIMEOUT, Group, StopPolicy},
    worker_observation::{Observations, Source},
};
use beamlet_agent::testing::{
    hello::Task,
    lifecycle::{ExitOutcome, Role, RoleNotice},
    worker_lifecycle::{self, Announcement},
};
use beamlet_message_zenoh::{Limits, Transport};
use serde::Deserialize;
use tokio::{fs, runtime::Handle, time::Duration};
use uuid::Uuid;

use crate::{lifecycle, roles};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    node_id: Uuid,
    #[serde(default = "default_drain_ms")]
    drain_timeout_ms: u64,
    #[serde(default)]
    listen: Vec<String>,
    #[serde(default)]
    connect: Vec<String>,
    #[serde(default)]
    authorities: Vec<AuthorityConfig>,
    #[serde(default)]
    workers: Vec<WorkerConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthorityConfig {
    id: Uuid,
    instance: Uuid,
    workers: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkerConfig {
    id: Uuid,
    instance: Uuid,
    authority: Uuid,
}

fn default_drain_ms() -> u64 {
    DEFAULT_DRAIN_TIMEOUT.as_secs() * 1000
}

impl Default for FileConfig {
    fn default() -> Self {
        let authority = Uuid::new_v4();
        let worker = Uuid::new_v4();
        Self {
            node_id: Uuid::new_v4(),
            drain_timeout_ms: default_drain_ms(),
            listen: vec!["tcp/127.0.0.1:7447".to_owned()],
            connect: Vec::new(),
            authorities: vec![AuthorityConfig {
                id: Uuid::new_v4(),
                instance: authority,
                workers: vec![worker],
            }],
            workers: vec![WorkerConfig {
                id: Uuid::new_v4(),
                instance: worker,
                authority,
            }],
        }
    }
}

/// 从可选 JSON 路径启动 Node, 无路径时默认承载一个 Authority 和一个本地 Worker
///
/// `listen` 只可覆盖默认配置的监听地址. 显式文件读取失败不会回退默认配置.
///
/// # 错误
/// 配置读取失败, 超限, 字段或角色绑定无效, 会话或角色驱动失败时返回错误
pub async fn run(path: Option<&str>, listen: Option<&str>) -> Result<()> {
    use tokio::io::AsyncReadExt;

    let (config, request_prefix) = if let Some(path) = path {
        ensure!(listen.is_none(), "--config 不能与 --listen 混用");
        let file = fs::File::open(path).await.context("打开 Node 配置失败")?;
        let mut bytes = Vec::new();
        file.take(64 * 1024 + 1).read_to_end(&mut bytes).await?;
        ensure!(bytes.len() <= 64 * 1024, "Node 配置超过 64 KiB");
        let config: FileConfig = serde_json::from_slice(&bytes).context("解析 Node 配置失败")?;
        (config, None)
    } else {
        let mut config = FileConfig::default();
        if let Some(endpoint) = listen {
            config.listen = vec![endpoint.to_owned()];
        }
        println!(
            "Node 使用默认配置: 1 个 Authority, 1 个 Worker; 本次逻辑身份随机生成, 不跨启动保留"
        );
        (config, Some(roles::NODE_INBOX_PREFIX))
    };
    let FileConfig {
        node_id,
        drain_timeout_ms,
        listen,
        connect,
        authorities,
        workers,
    } = config;
    let plan = composition::Config {
        node_id: node_id.as_u128().into(),
        drain_timeout: Duration::from_millis(drain_timeout_ms),
        authorities: authorities
            .into_iter()
            .map(|authority| composition::Authority {
                id: authority.id.as_u128().into(),
                instance: authority.instance.as_u128().into(),
                workers: authority
                    .workers
                    .into_iter()
                    .map(|worker| worker.as_u128().into())
                    .collect(),
            })
            .collect(),
        workers: workers
            .into_iter()
            .map(|worker| composition::Worker {
                id: worker.id.as_u128().into(),
                instance: worker.instance.as_u128().into(),
                authority: worker.authority.as_u128().into(),
            })
            .collect(),
    }
    .into_plan()?;
    let node_id = plan.node_id();
    let node_instance = NodeInstanceId::from_u128(Uuid::new_v4().as_u128());
    println!("Node 身份: 逻辑身份={node_id} 运行实例={node_instance}");
    let mut notices = Vec::new();
    for authority in plan.authorities() {
        notices.push(RoleNotice::new(
            Role::Authority,
            Uuid::from_u128(node_id.as_u128()),
            Uuid::from_u128(node_instance.as_u128()),
            Uuid::from_u128(authority.id.as_u128()),
            Uuid::from_u128(authority.instance.as_u128()),
        ));
        let id = authority.id;
        let instance = authority.instance;
        println!(
            "角色生命周期: Authority 状态=Starting; 逻辑身份={id} 运行实例={instance} NodeId={node_id} NodeInstanceId={node_instance}; 配置已校验, 尚未就绪"
        );
    }
    for worker in plan.workers() {
        notices.push(RoleNotice::new(
            Role::Worker,
            Uuid::from_u128(node_id.as_u128()),
            Uuid::from_u128(node_instance.as_u128()),
            Uuid::from_u128(worker.id.as_u128()),
            Uuid::from_u128(worker.instance.as_u128()),
        ));
        let id = worker.id;
        let instance = worker.instance;
        println!(
            "角色生命周期: Worker 状态=Starting; 逻辑身份={id} 运行实例={instance} NodeId={node_id} NodeInstanceId={node_instance}; 配置已校验, 尚未就绪"
        );
    }
    let subscription = format!("beamlet/v0/node-session/{node_instance}/inbox");
    let transport =
        Arc::new(Transport::node(&listen, &connect, &subscription, Limits::default()).await?);
    let mut drivers = Group::new(Handle::current(), plan.drain_timeout())?;
    // 先声明所有订阅再启动角色, 避免共享会话中的局部启动顺序影响就绪
    let mut authorities = Vec::new();
    let mut workers = Vec::new();
    for authority in plan.authorities() {
        let prefix = match request_prefix {
            Some(prefix) => prefix.to_owned(),
            None => format!(
                "beamlet/v0/authority/{}/submit/",
                Uuid::from_u128(authority.instance.as_u128()).simple()
            ),
        };
        let requests = transport.subscribe(&format!("{prefix}*")).await?;
        let instance: RoleInstanceId = authority.instance.into();
        let reports = transport.subscribe(&roles::report_inbox(instance)).await?;
        // 两种地址各有独立有界队列, 由同一 Authority 驱动处理
        let combined = (requests, reports);
        let announcements = transport
            .subscribe(&worker_lifecycle::inbox(Uuid::from_u128(
                authority.instance.as_u128(),
            )))
            .await?;
        let mut observations =
            Observations::new(authority.instance, plan.worker_slots(authority.instance))?;
        let mut local = Vec::new();
        let mut local_announcements = Vec::new();
        for worker in plan.local_workers(authority.instance) {
            let worker_id: RoleInstanceId = worker.instance.into();
            local.push(worker_id);
            let announcement = Announcement::started(
                Uuid::from_u128(node_id.as_u128()),
                Uuid::from_u128(node_instance.as_u128()),
                Uuid::from_u128(worker.id.as_u128()),
                Uuid::from_u128(worker.instance.as_u128()),
                Uuid::from_u128(worker.authority.as_u128()),
            );
            lifecycle::record(&mut observations, announcement.clone(), Source::Local)?;
            local_announcements.push(announcement);
        }
        authorities.push((
            combined,
            authority.clone(),
            local,
            prefix,
            (observations, announcements, local_announcements),
        ));
    }
    for worker in plan.remote_workers() {
        let inbox = roles::worker_inbox(worker.instance.as_u128().into());
        let receipts = beamlet_agent::testing::receipt::inbox(worker.instance.as_u128().into());
        workers.push((
            transport.subscribe(&inbox).await?,
            transport.subscribe(&receipts).await?,
            *worker,
        ));
    }
    let driver_count = authorities.len() + workers.len();
    for (
        (requests, reports),
        authority,
        local,
        prefix,
        (observations, announcements, local_announcements),
    ) in authorities
    {
        let mut instances: Vec<Uuid> = local
            .iter()
            .map(|id| Uuid::from_u128(id.as_u128()))
            .collect();
        instances.push(Uuid::from_u128(authority.instance.as_u128()));
        let local_worker = if local.is_empty() {
            None
        } else {
            Some(Worker::<Task, String, String>::new(
                authority.instance.as_u128().into(),
                local,
                beamlet::memory::Limits::default(),
            )?)
        };
        drivers.start(instances, StopPolicy::Drain, |control| async move {
            tokio::select! {
                result = roles::serve_authority_pair(
                    Arc::new(requests),
                    Some(reports),
                    roles::AuthoritySetup {
                        instance: authority.instance.as_u128().into(),
                        workers: authority.workers.into_iter().map(|id| id.as_u128().into()).collect(),
                        local_worker,
                        request_prefix: prefix,
                    },
                    control,
                ) => result,
                result = lifecycle::observe(observations, announcements, local_announcements) => result,
            }
        })?;
    }
    for (worker_transport, receipt_transport, worker) in workers {
        let announcement = Announcement::started(
            Uuid::from_u128(node_id.as_u128()),
            Uuid::from_u128(node_instance.as_u128()),
            Uuid::from_u128(worker.id.as_u128()),
            Uuid::from_u128(worker.instance.as_u128()),
            Uuid::from_u128(worker.authority.as_u128()),
        );
        let publisher = Arc::clone(&transport);
        drivers.start(
            vec![Uuid::from_u128(worker.instance.as_u128())],
            StopPolicy::Drain,
            |control| async move {
                tokio::select! {
                    result = lifecycle::maintain(&publisher, announcement) => result,
                    result = roles::serve_worker(
                        worker_transport,
                        receipt_transport,
                        worker.instance.as_u128().into(),
                        worker.authority.as_u128().into(),
                        control,
                    ) => result,
                }
            },
        )?;
    }
    let role_count = notices.len();
    for notice in &notices {
        println!(
            "角色生命周期: {} 状态=Running; 逻辑身份={} 运行实例={} NodeId={} NodeInstanceId={}; 本地驱动已启动, 不代表远端可达",
            notice.role,
            notice.role_id,
            notice.role_instance_id,
            notice.node_id,
            notice.node_instance_id
        );
    }
    let mut outcomes: HashMap<Uuid, ExitOutcome> = notices
        .iter()
        .map(|notice| (notice.role_instance_id, ExitOutcome::Unconfirmed))
        .collect();
    if !notices.is_empty() {
        let publisher = Arc::clone(&transport);
        let starts = notices.clone();
        drivers.start(Vec::new(), StopPolicy::Abort, |_| async move {
            lifecycle::publish_starts(&publisher, starts).await
        })?;
    }
    println!("Node 已就绪: 消息会话共享; 角色数量={role_count}; 驱动数量={driver_count}");
    let mut failure = tokio::select! {
        _ = drivers.wait() => None,
        signal = tokio::signal::ctrl_c() => signal.context("接收退出信号失败").err(),
    };
    drivers.stop();
    println!(
        "Node 进入 Draining: 停止新接纳; 等待上限={} 毫秒",
        plan.drain_timeout().as_millis()
    );
    for notice in &notices {
        println!(
            "角色生命周期: {} 请求状态=Draining; 逻辑身份={} 运行实例={} NodeId={} NodeInstanceId={}; Node 已请求停止新接纳, 不代表角色已完成排空",
            notice.role,
            notice.role_id,
            notice.role_instance_id,
            notice.node_id,
            notice.node_instance_id
        );
    }
    let summary = drivers.drain().await;
    for exit in summary.exits {
        match exit.result {
            Ok(Completion::Drained) => {
                for instance in exit.label {
                    outcomes.insert(instance, ExitOutcome::Drained);
                }
            }
            Ok(Completion::Aborted) => {}
            Err(error) => {
                eprintln!("Draining 驱动未正常完成: {error}; 不生成失败 Report");
                if failure.is_none() {
                    failure = Some(anyhow::anyhow!("{error}"));
                }
            }
        }
    }
    if summary.timed_out {
        eprintln!(
            "Node Draining 到期: 执行或交付未确认, 不生成失败 Report; 已中止并收回剩余驱动, 内存状态不保证恢复"
        );
        failure = Some(anyhow::anyhow!("Node Draining 到期, 未确认执行或交付"));
    }
    for notice in &notices {
        let outcome = outcomes[&notice.role_instance_id];
        println!(
            "角色生命周期: {} 状态=Stopped; 逻辑身份={} 运行实例={} NodeId={} NodeInstanceId={}; 排空观察={outcome}; 本地驱动已结束, 不证明外部效果停止",
            notice.role,
            notice.role_id,
            notice.role_instance_id,
            notice.node_id,
            notice.node_instance_id
        );
    }
    lifecycle::publish_exits(&transport, notices, &outcomes).await;
    if let Some(error) = failure {
        return Err(error);
    }
    println!("Node Draining 完成: 角色已停止, 随后关闭消息会话; 不提供跨重启恢复");
    Ok(())
}
