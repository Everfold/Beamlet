//! 普通行式交互, 编号只用于显示, 网络目标始终保存完整实例身份.
use crate::settings::Settings;
use anyhow::{Context, Result, bail, ensure};
use beamlet_api::{
    AuthorityInfo, Command, Outcome, Pool, PresenceState, RoleInstance, Selection, Submission,
    Task, WorkerView,
};
use std::{collections::BTreeSet, future::Future, io::BufRead, time::Duration};
use tokio::{sync::mpsc, task::JoinSet};
use tracing::Instrument;
use uuid::Uuid;

#[derive(Default)]
struct Choice {
    authority: Option<RoleInstance>,
    pool: Option<Pool>,
    capability: Option<String>,
    worker: Option<Selection>,
}
struct Session {
    pools: BTreeSet<Pool>,
    max_in_flight_requests: usize,
    authorities: Vec<AuthorityInfo>,
    workers: Vec<WorkerView>,
    capabilities: Vec<String>,
    choice: Choice,
    choice_revision: Uuid,
    discovery_request: Option<Uuid>,
    workers_request: Option<Uuid>,
}

enum Action {
    Discover {
        request_id: Uuid,
        pools: BTreeSet<Pool>,
    },
    Request {
        command: Command,
        choice_revision: Uuid,
    },
}
enum Completion {
    Discovered {
        request_id: Uuid,
        result: Result<Vec<AuthorityInfo>>,
    },
    Workers {
        request_id: Uuid,
        authority: RoleInstance,
        pool: Pool,
        choice_revision: Uuid,
        result: Result<Vec<WorkerView>>,
    },
    Submitted {
        request_id: Uuid,
        choice_revision: Uuid,
        result: Result<Outcome>,
    },
}
impl Completion {
    fn reply(command: Command, choice_revision: Uuid, result: Result<Outcome>) -> Self {
        match command {
            Command::Workers {
                request_id,
                authority,
                pool,
                ..
            } => Self::Workers {
                request_id,
                authority,
                pool,
                choice_revision,
                result: result.and_then(|outcome| match outcome {
                    Outcome::Workers { workers } => Ok(workers),
                    outcome => bail!("Worker 查询被拒绝: {outcome:?}"),
                }),
            },
            Command::Submit { request_id, .. } => Self::Submitted {
                request_id,
                choice_revision,
                result,
            },
        }
    }
}
impl Action {
    fn request_id(&self) -> Uuid {
        match self {
            Self::Discover { request_id, .. } => *request_id,
            Self::Request { command, .. } => command.request_id(),
        }
    }
    async fn execute(
        self,
        client: beamlet_client::Client<beamlet_transport_nats::Nats>,
    ) -> Completion {
        match self {
            Self::Discover { request_id, pools } => Completion::Discovered {
                // UI 动作身份独立于库内部的发现请求, 用于屏蔽旧查询结果.
                request_id,
                result: client.discover(&pools).await.context("发现 Authority 失败"),
            },
            Self::Request {
                command,
                choice_revision,
            } => {
                let result = client
                    .request(command.clone())
                    .await
                    .context("请求未取得有效回复; 可能已执行, 不会自动重发");
                Completion::reply(command, choice_revision, result)
            }
        }
    }
}

impl Session {
    fn new(pools: BTreeSet<Pool>, max_in_flight_requests: usize) -> Self {
        Self {
            pools,
            max_in_flight_requests,
            authorities: Vec::new(),
            workers: Vec::new(),
            capabilities: Vec::new(),
            choice: Choice::default(),
            choice_revision: Uuid::new_v4(),
            discovery_request: None,
            workers_request: None,
        }
    }
    fn refresh(&self) -> Action {
        Action::Discover {
            request_id: Uuid::new_v4(),
            pools: self.pools.clone(),
        }
    }
    fn discovered(&mut self, authorities: Vec<AuthorityInfo>) {
        self.authorities = authorities;
        println!("Authority 列表:");
        for (index, info) in self.authorities.iter().enumerate() {
            println!(
                "  {}. {}  role={}  session={}  pools={:?}",
                index + 1,
                info.label.as_deref().unwrap_or("未命名"),
                info.instance.role_id,
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
            self.choice_revision = Uuid::new_v4();
            self.workers_request = None;
            self.workers.clear();
            self.capabilities.clear();
            println!("原 Authority 会话已不可见, 请重新选择");
        }
        println!(
            "/authority <编号> → /pool <编号或名称> → /capability <编号或标识> → /worker <编号或 any>"
        );
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
    fn workers(&self) -> Result<Action> {
        Ok(Action::Request {
            command: Command::Workers {
                request_id: Uuid::new_v4(),
                authority: self.authority()?.instance,
                pool: self
                    .choice
                    .pool
                    .clone()
                    .context("请先 /pool <编号或名称>")?,
                capability: None,
            },
            choice_revision: self.choice_revision,
        })
    }
    fn update_workers(&mut self, workers: Vec<WorkerView>) {
        let previous = (self.choice.capability.clone(), self.choice.worker);
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
        if previous != (self.choice.capability.clone(), self.choice.worker) {
            self.choice_revision = Uuid::new_v4();
        }
        self.show_workers();
        println!("能力列表:");
        for (index, capability) in self.capabilities.iter().enumerate() {
            println!("  {}. {capability}", index + 1);
        }
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
                "  {}. {}  role={}  session={}  pool={}  {status}  心跳={}ms  {:?}",
                index + 1,
                presence.label.as_deref().unwrap_or("未命名"),
                presence.instance.role_id,
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
    fn line(&mut self, line: String) -> Result<Option<Action>> {
        if line == "/refresh" {
            return Ok(Some(self.refresh()));
        }
        if line == "/workers" {
            return self.workers().map(Some);
        }
        if line == "/help" {
            println!(
                "/refresh 发现 Authority; /workers 刷新 Worker; /authority N; /pool N或名称; /capability N或标识; /worker N或any; /text 文本; /quit"
            );
            return Ok(None);
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
            self.choice_revision = Uuid::new_v4();
            self.workers_request = None;
            self.workers.clear();
            self.capabilities.clear();
            println!("Pool 列表:");
            for (index, pool) in self.available_pools()?.iter().enumerate() {
                println!("  {}. {pool}", index + 1);
            }
            return Ok(None);
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
            self.choice_revision = Uuid::new_v4();
            self.workers_request = None;
            self.workers.clear();
            self.capabilities.clear();
            return self.workers().map(Some);
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
            self.choice_revision = Uuid::new_v4();
            self.show_workers();
            println!("/worker <编号或 any> 选择目标");
            return Ok(None);
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
            self.choice_revision = Uuid::new_v4();
            println!("已选择 {selection:?}; 输入文本后回车提交, /text 可输入以 / 开头的文本");
            return Ok(None);
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
                payload: text,
            },
        };
        Ok(Some(Action::Request {
            command: Command::Submit {
                request_id: Uuid::new_v4(),
                authority,
                submission,
            },
            choice_revision: self.choice_revision,
        }))
    }
    fn submitted(
        &mut self,
        request_id: Uuid,
        choice_revision: Uuid,
        outcome: Outcome,
    ) -> Result<()> {
        match outcome {
            Outcome::Completed {
                operation_id,
                attempt_id,
                report_id,
                worker,
                result,
            } => {
                tracing::info!(event = "response.received", %operation_id, %attempt_id, %report_id, role_id = %worker.role_id, session_id = %worker.session_id, "Operation 已完成");
                println!("[{request_id}] {result}");
            }
            Outcome::Rejected { reason } => {
                if self.choice_revision == choice_revision {
                    self.choice.worker = None;
                    self.choice_revision = Uuid::new_v4();
                }
                println!("[{request_id}] 未创建 Operation: {reason}; /workers 可刷新列表");
            }
            Outcome::Unresolved {
                operation_id,
                attempt_id,
                worker,
                reason,
            } => {
                tracing::warn!(event = "operation.unresolved", %operation_id, %attempt_id, role_id = %worker.role_id, session_id = %worker.session_id, %reason, "操作保持等待报告, 未重试");
            }
            Outcome::Workers { .. } => bail!("提交收到了不匹配的回复类型"),
        }
        Ok(())
    }
    fn complete(&mut self, completion: Completion) -> Result<()> {
        match completion {
            Completion::Discovered { request_id, result } => {
                if self.discovery_request == Some(request_id) {
                    self.discovery_request = None;
                    self.discovered(result?);
                }
            }
            Completion::Workers {
                request_id,
                authority,
                pool,
                choice_revision,
                result,
            } => {
                if self.workers_request == Some(request_id) {
                    self.workers_request = None;
                    if self.choice.authority == Some(authority)
                        && self.choice.pool.as_ref() == Some(&pool)
                        && self.choice_revision == choice_revision
                    {
                        self.update_workers(result?);
                    }
                }
            }
            Completion::Submitted {
                request_id,
                choice_revision,
                result,
            } => {
                self.submitted(request_id, choice_revision, result?)?;
            }
        }
        Ok(())
    }
    fn start<F: Future<Output = Completion> + Send + 'static>(
        &mut self,
        action: Action,
        tasks: &mut JoinSet<(Uuid, Completion)>,
        execute: &impl Fn(Action) -> F,
    ) -> Result<()> {
        ensure!(
            tasks.len() < self.max_in_flight_requests,
            "已有 {} 个在途请求, 本次请求未发送, 请稍后再试",
            self.max_in_flight_requests
        );
        let request_id = action.request_id();
        match &action {
            Action::Discover { .. } => self.discovery_request = Some(request_id),
            Action::Request {
                command: Command::Workers { .. },
                ..
            } => self.workers_request = Some(request_id),
            _ => {}
        }
        let future = execute(action);
        tasks.spawn(async move { (request_id, future.await) }.in_current_span());
        tracing::info!(event = "request.started", %request_id, "请求已开始等待");
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
    settings.validate()?;
    let connect_timeout = Duration::from_secs(settings.connect_timeout_secs);
    let client_options = (&settings).into();
    let client = beamlet_transport_nats::Nats::connect(
        &settings.nats_url,
        beamlet_transport_nats::Options {
            connect_timeout,
            subscription_capacity: settings.subscription_capacity,
        },
    )
    .await
    .context("连接 NATS 失败")?;
    let client = beamlet_client::Client::new(client, client_options)?;
    let mut session = Session::new(settings.pools, settings.max_in_flight_requests);
    let work = drive(&mut session, terminal_input(), move |action| {
        action.execute(client.clone())
    });
    tokio::select! {
        result = work => result?,
        signal = tokio::signal::ctrl_c() => {
            signal?;
            tracing::info!(event = "console.interrupted", "停止等待回复; 已发送请求可能继续执行, 不会自动重发");
        }
    }
    tracing::info!(event = "console.stopped", "Console 已退出");
    Ok(())
}

async fn drive<F: Future<Output = Completion> + Send + 'static>(
    session: &mut Session,
    mut input: mpsc::Receiver<std::io::Result<String>>,
    execute: impl Fn(Action) -> F,
) -> Result<()> {
    let mut tasks = JoinSet::new();
    session.start(session.refresh(), &mut tasks, &execute)?;
    tracing::info!(event = "console.ready", "Console 已连接; /help 查看命令");
    let mut stopping = false;
    while !stopping || !tasks.is_empty() {
        tokio::select! {
            // 先回收完成的请求, 满载也始终读取本地输入和退出命令.
            biased;
            completed = tasks.join_next(), if !tasks.is_empty() => {
                let (request_id, completion) = completed.context("缺少 Console 请求任务")?.context("Console 请求任务异常退出")?;
                let span = tracing::info_span!("request", %request_id);
                span.in_scope(|| {
                    if let Err(error) = session.complete(completion) {
                        tracing::warn!(event = "request.failed", error = %format!("{error:#}"), "请求未完成");
                    }
                });
            }
            line = input.recv(), if !stopping => {
                let line = line.transpose().context("读取输入失败")?;
                if line.as_deref().is_none_or(|line| line == "/quit") {
                    stopping = true;
                    input.close();
                    tracing::info!(event = "console.draining", pending = tasks.len(), "停止新请求, 等待在途回复; Ctrl+C 可停止等待");
                    continue;
                }
                let result = session.line(line.context("缺少输入行")?).and_then(|action| {
                    if let Some(action) = action { session.start(action, &mut tasks, &execute)?; }
                    Ok(())
                });
                if let Err(error) = result {
                    tracing::warn!(event = "request.failed", error = %format!("{error:#}"), "输入未完成");
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/unit/session.rs"]
mod tests;

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
