//! 普通行式交互, 编号只用于显示, 网络目标始终保存完整实例身份.
use crate::{client, settings::Settings};
use anyhow::{Context, Result, bail, ensure};
use beamlet_protocol::{
    AuthorityInfo, Command, Instance, Outcome, Pool, PresenceState, REQUEST_TIMEOUT, Selection,
    Submission, Task, WorkerView,
};
use std::{collections::BTreeSet, io::BufRead};
use tokio::sync::mpsc;
use uuid::Uuid;

#[derive(Default)]
struct Choice {
    authority: Option<Instance>,
    pool: Option<Pool>,
    capability: Option<String>,
    worker: Option<Selection>,
}
struct Session {
    client: async_nats::Client,
    pools: BTreeSet<Pool>,
    authorities: Vec<AuthorityInfo>,
    workers: Vec<WorkerView>,
    capabilities: Vec<String>,
    choice: Choice,
}
impl Session {
    async fn refresh(&mut self) -> Result<()> {
        self.authorities = client::discover(&self.client, &self.pools).await?;
        println!("Authority 列表:");
        for (index, info) in self.authorities.iter().enumerate() {
            println!(
                "  {}. {}  node={}  session={}  pools={:?}",
                index + 1,
                info.label.as_deref().unwrap_or("未命名"),
                info.instance.node_id,
                info.instance.session_id,
                info.pools
            );
        }
        if self.authorities.is_empty() {
            println!("  暂无 Authority; /refresh 重新发现");
        }
        if self.choice.authority.is_some_and(|selected| {
            !self
                .authorities
                .iter()
                .any(|info| info.instance == selected)
        }) {
            self.choice = Choice::default();
            self.workers.clear();
            self.capabilities.clear();
            println!("原 Authority 会话已不可见, 请重新选择");
        }
        println!(
            "/authority <编号> → /pool <编号或名称> → /capability <编号或标识> → /worker <编号或 any>"
        );
        Ok(())
    }
    fn authority(&self) -> Result<&AuthorityInfo> {
        self.authorities
            .iter()
            .find(|info| Some(info.instance) == self.choice.authority)
            .context("请先 /authority <编号>")
    }
    fn available_pools(&self) -> Result<Vec<Pool>> {
        Ok(self
            .authority()?
            .pools
            .intersection(&self.pools)
            .cloned()
            .collect())
    }
    async fn workers(&mut self) -> Result<()> {
        let authority = self.authority()?.instance;
        let pool = self
            .choice
            .pool
            .clone()
            .context("请先 /pool <编号或名称>")?;
        let outcome = client::request(
            &self.client,
            Command::Workers {
                request_id: Uuid::new_v4(),
                authority,
                pool,
                capability: None,
            },
        )
        .await?;
        let Outcome::Workers { workers } = outcome else {
            bail!("Worker 查询被拒绝: {outcome:?}");
        };
        self.workers = workers;
        self.capabilities = self
            .workers
            .iter()
            .filter(|worker| available(worker))
            .flat_map(|worker| worker.presence.capabilities.iter().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if self
            .choice
            .capability
            .as_ref()
            .is_some_and(|cap| !self.capabilities.contains(cap))
        {
            self.choice.capability = None;
            self.choice.worker = None;
        }
        if let Some(Selection::Exact(instance)) = self.choice.worker
            && !self.workers.iter().any(|worker| {
                worker.presence.instance == instance
                    && available(worker)
                    && self
                        .choice
                        .capability
                        .as_ref()
                        .is_some_and(|cap| worker.presence.capabilities.contains(cap))
            })
        {
            self.choice.worker = None;
            println!("原 Worker 会话不可用, 请刷新并重新选择");
        }
        self.show_workers();
        println!("能力列表:");
        for (index, capability) in self.capabilities.iter().enumerate() {
            println!("  {}. {capability}", index + 1);
        }
        Ok(())
    }
    fn show_workers(&self) {
        println!("Worker 列表:");
        for (index, worker) in self.workers.iter().enumerate() {
            let presence = &worker.presence;
            let status = if worker.stale {
                "失联"
            } else {
                match presence.state {
                    PresenceState::Ready => "ready",
                    PresenceState::Draining => "draining",
                    PresenceState::Offline => "offline",
                }
            };
            println!(
                "  {}. {}  node={}  session={}  pool={}  {status}  心跳={}ms  {:?}",
                index + 1,
                presence.label.as_deref().unwrap_or("未命名"),
                presence.instance.node_id,
                presence.instance.session_id,
                presence.pool,
                worker.last_seen_ms,
                presence.capabilities
            );
        }
        if self.workers.is_empty() {
            println!("  暂无 Worker; /workers 刷新");
        }
    }
    async fn line(&mut self, line: String) -> Result<()> {
        if line == "/refresh" {
            return self.refresh().await;
        }
        if line == "/workers" {
            return self.workers().await;
        }
        if line == "/help" {
            println!(
                "/refresh 发现 Authority; /workers 刷新 Worker; /authority N; /pool N或名称; /capability N或标识; /worker N或any; /text 文本; /quit"
            );
            return Ok(());
        }
        if let Some(arg) = line.strip_prefix("/authority ") {
            let instance = self
                .authorities
                .get(index(arg)?)
                .context("Authority 编号不存在")?
                .instance;
            self.choice = Choice {
                authority: Some(instance),
                ..Choice::default()
            };
            self.workers.clear();
            self.capabilities.clear();
            println!("Pool 列表:");
            for (index, pool) in self.available_pools()?.iter().enumerate() {
                println!("  {}. {pool}", index + 1);
            }
            return Ok(());
        }
        if let Some(arg) = line.strip_prefix("/pool ") {
            let pools = self.available_pools()?;
            let pool = match arg.parse::<usize>() {
                Ok(_) => pools.get(index(arg)?).context("Pool 编号不存在")?.clone(),
                Err(_) => arg.parse()?,
            };
            ensure!(
                pools.contains(&pool),
                "该 pool 不属于选定 Authority 或 Console 的发现范围"
            );
            self.choice.pool = Some(pool);
            self.choice.capability = None;
            self.choice.worker = None;
            return self.workers().await;
        }
        if let Some(arg) = line.strip_prefix("/capability ") {
            let capability = match arg.parse::<usize>() {
                Ok(_) => self
                    .capabilities
                    .get(index(arg)?)
                    .context("能力编号不存在")?
                    .clone(),
                Err(_) => arg.to_owned(),
            };
            ensure!(
                self.capabilities.contains(&capability),
                "没有可用 Worker 声明该能力; /workers 刷新"
            );
            self.choice.capability = Some(capability);
            self.choice.worker = None;
            self.show_workers();
            println!("/worker <编号或 any> 选择目标");
            return Ok(());
        }
        if let Some(arg) = line.strip_prefix("/worker ") {
            let capability = self
                .choice
                .capability
                .as_ref()
                .context("请先 /capability <编号或标识>")?;
            let selection = if arg == "any" {
                Selection::Any
            } else {
                let worker = self.workers.get(index(arg)?).context("Worker 编号不存在")?;
                ensure!(
                    available(worker) && worker.presence.capabilities.contains(capability),
                    "Worker 不可用或能力不匹配; /workers 刷新"
                );
                Selection::Exact(worker.presence.instance)
            };
            self.choice.worker = Some(selection);
            println!("已选择 {selection:?}; 输入文本后回车提交, /text 可输入以 / 开头的文本");
            return Ok(());
        }
        let text = if let Some(text) = line.strip_prefix("/text ") {
            text.to_owned()
        } else {
            ensure!(!line.starts_with('/'), "未知命令; /help 查看帮助");
            line
        };
        let authority = self.authority()?.instance;
        let submission = Submission {
            pool: self.choice.pool.clone().context("请先选择 pool")?,
            worker: self.choice.worker.context("请先选择 Worker")?,
            task: Task {
                capability: self.choice.capability.clone().context("请先选择能力")?,
                text,
            },
        };
        let outcome = client::request(
            &self.client,
            Command::Submit {
                request_id: Uuid::new_v4(),
                authority,
                submission,
            },
        )
        .await?;
        match outcome {
            Outcome::Completed {
                operation_id,
                attempt_id,
                report_id,
                worker,
                result,
            } => {
                tracing::info!(event = "response.received", %operation_id, %attempt_id, %report_id, node_id = %worker.node_id, session_id = %worker.session_id, "Operation 已完成");
                println!("{result}");
            }
            Outcome::Rejected { reason } => {
                self.choice.worker = None;
                println!("未创建 Operation: {reason}; /workers 刷新后重新选择");
            }
            Outcome::Unresolved {
                operation_id,
                attempt_id,
                worker,
                reason,
            } => {
                tracing::warn!(event = "operation.unresolved", %operation_id, %attempt_id, node_id = %worker.node_id, session_id = %worker.session_id, %reason, "操作保持等待报告, 未重试");
            }
            Outcome::Workers { .. } => bail!("提交收到了不匹配的回复类型"),
        }
        Ok(())
    }
}
fn available(worker: &WorkerView) -> bool {
    !worker.stale && worker.presence.state == PresenceState::Ready
}
fn index(input: &str) -> Result<usize> {
    input
        .parse::<usize>()?
        .checked_sub(1)
        .context("编号从 1 开始")
}

pub(crate) async fn run(settings: Settings) -> Result<()> {
    tracing::info!(event = "console.starting", "Console 启动");
    let client = async_nats::ConnectOptions::new()
        .request_timeout(Some(REQUEST_TIMEOUT))
        .connect(settings.nats_url)
        .await
        .context("连接 NATS 失败")?;
    let mut session = Session {
        client,
        pools: settings.pools,
        authorities: Vec::new(),
        workers: Vec::new(),
        capabilities: Vec::new(),
        choice: Choice::default(),
    };
    let mut input = terminal_input();
    let work = async {
        session.refresh().await?;
        tracing::info!(event = "console.ready", "Console 已连接; /help 查看命令");
        while let Some(line) = input.recv().await {
            let line = line.context("读取输入失败")?;
            if line == "/quit" {
                break;
            }
            if let Err(error) = session.line(line).await {
                tracing::warn!(event = "request.failed", error = %format!("{error:#}"), "请求未完成");
            }
        }
        Ok::<_, anyhow::Error>(())
    };
    tokio::select! { result = work => result?, signal = tokio::signal::ctrl_c() => signal? }
    tracing::info!(event = "console.stopped", "Console 已退出");
    Ok(())
}
fn terminal_input() -> mpsc::Receiver<std::io::Result<String>> {
    let (send, recv) = mpsc::channel(16);
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            if send.blocking_send(line).is_err() {
                break;
            }
        }
    });
    recv
}
