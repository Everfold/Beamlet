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

/// Console 当前选择的执行目标；可逐步选择，尚未指定的项为 None。
#[derive(Default)]
struct Choice {
    /// 当前选定的 Authority 角色会话；None 表示尚未选择。
    authority: Option<RoleInstance>,
    /// 当前选定的 pool；切换 Authority 后需要重新选择。
    pool: Option<Pool>,
    /// 当前选定的能力名称，必须由可用 Worker 声明。
    capability: Option<String>,
    /// 当前 Worker 选择策略；None 表示尚未明确选择。
    worker: Option<Selection>,
}
/// Console 的交互状态与在途请求关联信息；不持有 Operation 台账。
struct Session {
    /// Console 允许发现和选择的 pool 集合。
    pools: BTreeSet<Pool>,
    /// 终端网络动作的在途上限，包括发现、查询和提交，必须为正数。
    max_in_flight_requests: usize,
    /// 最近一次被接受的 Authority 发现列表。
    authorities: Vec<AuthorityInfo>,
    /// 当前 Authority/pool 的最近一次有效 Worker 查询结果。
    workers: Vec<WorkerView>,
    /// 从未过期 Ready Worker 汇总出的可选能力名称。
    capabilities: Vec<String>,
    /// 当前交互选择，供下一次动作创建不可变目标快照。
    choice: Choice,
    /// 动作创建时的选择版本；完成事件不能覆盖后来的选择。
    choice_revision: Uuid,
    /// 最新发现动作的身份；只接受与它匹配的发现结果。
    discovery_request: Option<Uuid>,
    /// 最新 Worker 查询身份；配合选择版本屏蔽迟到结果。
    workers_request: Option<Uuid>,
}

/// 尚未执行的 Console 网络动作，保存发起时的目标快照。
enum Action {
    /// 查询配置范围内可见的 Authority 实例。
    Discover {
        /// Console 动作的关联 UUID，用于屏蔽旧结果。
        request_id: Uuid,
        /// 本次 Console 刷新动作保存的发现 pool 范围。
        pools: BTreeSet<Pool>,
    },
    /// 执行已保存目标快照的 Worker 查询或业务提交。
    Request {
        /// 目标 Authority 的查询或提交命令。
        command: Command,
        /// 动作创建时的选择版本；完成事件不能覆盖后来的选择。
        choice_revision: Uuid,
    },
}
/// 异步网络动作的完成事件；交互层按请求身份和选择版本决定是否应用结果。
enum Completion {
    /// 一次 Authority 发现完成，可能携带查询错误。
    Discovered {
        /// Console 动作的关联 UUID，用于屏蔽旧结果。
        request_id: Uuid,
        /// 发现得到的 Authority 列表，或该次调用错误。
        result: Result<Vec<AuthorityInfo>>,
    },
    /// 一次 Worker 查询完成，保留当时的 Authority、pool 和选择版本。
    Workers {
        /// Console 动作的关联 UUID，用于屏蔽旧结果。
        request_id: Uuid,
        /// 明确指定或实际应答的 Authority 角色与会话身份。
        authority: RoleInstance,
        /// 本次查询或调用所属的 pool，按精确名称隔离。
        pool: Pool,
        /// 动作创建时的选择版本；完成事件不能覆盖后来的选择。
        choice_revision: Uuid,
        /// Worker 快照列表，或查询失败原因。
        result: Result<Vec<WorkerView>>,
    },
    /// 一次业务提交完成，保留请求身份和创建时的选择版本。
    Submitted {
        /// Console 动作的关联 UUID，用于屏蔽旧结果。
        request_id: Uuid,
        /// 动作创建时的选择版本；完成事件不能覆盖后来的选择。
        choice_revision: Uuid,
        /// 提交的业务结果或客户端调用错误。
        result: Result<Outcome>,
    },
}
impl Completion {
    /// 将命令及其异步结果转换为携带选择版本的完成事件。
    ///
    /// # 参数
    ///
    /// - `command`：本次 Authority 命令，携带请求 UUID、目标会话及查询/提交数据。
    /// - `choice_revision`：创建动作时的选择版本，用于防止旧结果覆盖后来的选择。
    /// - `result`：调用已完成的 Outcome 或客户端错误，转换后仍保留在完成事件中。
    ///
    /// # 返回值
    ///
    /// 与命令对应的 Worker 查询或提交完成事件；错误保存在事件中。
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
    /// 读取请求与回复关联使用的 UUID。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Action` 的共享引用。
    ///
    /// # 返回值
    ///
    /// 当前动作或命令已有的请求 UUID，不生成新身份。
    fn request_id(&self) -> Uuid {
        match self {
            Self::Discover { request_id, .. } => *request_id,
            Self::Request { command, .. } => command.request_id(),
        }
    }
    /// 执行一次发现或请求动作，并保留界面所需的关联信息。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Action` 的值；调用后转移其所有权（Copy 类型按值复制）。
    /// - `client`：已经装配好的调用客户端，用于执行当前动作。
    ///
    /// # 返回值
    ///
    /// 完成事件；包含成功结果或带上下文的调用错误。
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
    /// 创建尚未选择目标的 Console 会话。
    ///
    /// # 参数
    ///
    /// - `pools`：本次发现或登记关注的 pool 集合，按精确名称匹配。
    /// - `max_in_flight_requests`：终端允许同时等待的网络动作数，必须大于零。
    ///
    /// # 返回值
    ///
    /// 空发现列表和默认选择状态的会话，使用给定的在途上限。
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
    /// 创建重新发现 Authority 的动作。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Session` 的共享引用。
    ///
    /// # 返回值
    ///
    /// 包含新请求 UUID 和当前 pool 集合的发现动作；此时尚未发送。
    fn refresh(&self) -> Action {
        Action::Discover {
            request_id: Uuid::new_v4(),
            pools: self.pools.clone(),
        }
    }
    /// 更新并显示 Authority 列表，清除已失效的选择。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Session` 的可变引用。
    /// - `authorities`：此次发现得到的 Authority 描述列表。
    ///
    /// # 返回值
    ///
    /// `()`；新列表写入会话，必要时重置选择和旧查询状态。
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
    /// 查找当前明确选定的 Authority。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Session` 的共享引用。
    ///
    /// # 返回值
    ///
    /// 成功返回列表中对应描述的借用；尚未选择或目标已消失时返回错误。
    fn authority(&self) -> Result<&AuthorityInfo> {
        self.authorities
            .iter()
            .find(|info| Some(info.instance) == self.choice.authority)
            .context("请先 /authority <编号>")
    }
    /// 计算当前 Authority 与 Console 共同允许的 pools。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Session` 的共享引用。
    ///
    /// # 返回值
    ///
    /// 成功返回两者 pool 集合的交集；Authority 尚未选择时返回错误。
    fn available_pools(&self) -> Result<Vec<Pool>> {
        Ok(self
            .authority()?
            .pools
            .intersection(&self.pools)
            .cloned()
            .collect())
    }
    /// 为当前目标创建 Worker 查询动作。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Session` 的共享引用。
    ///
    /// # 返回值
    ///
    /// 携带新 UUID 和选择版本的动作；缺少 Authority 或 pool 时返回错误。
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
    /// 更新 Worker 快照与能力列表，并清除不再可用的选择。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Session` 的可变引用。
    /// - `workers`：此次查询得到的 Worker 快照列表，可能包含失联或下线记录。
    ///
    /// # 返回值
    ///
    /// `()`；更新会话列表、选择版本并输出 Worker 与能力信息。
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
    /// 显示 Worker 的标签、身份、pool、状态和心跳年龄。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Session` 的共享引用。
    ///
    /// # 返回值
    ///
    /// `()`；信息写入终端，不改变选择。
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
    /// 解析一行交互输入，更新选择或创建网络动作。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Session` 的可变引用。
    /// - `line`：用户输入的一行文本，尚未解析为选择命令或业务任务。
    ///
    /// # 返回值
    ///
    /// `Ok(Some(action))` 表示需要执行网络动作；`Ok(None)` 表示已完成本地操作；输入无效或选择不完整时返回错误。
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
    /// 展示提交结果，仅让当前选择对应的拒绝影响选择状态。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Session` 的可变引用。
    /// - `request_id`：请求与回复关联 UUID；不作为 Operation 身份或幂等凭据。
    /// - `choice_revision`：创建动作时的选择版本，用于防止旧结果覆盖后来的选择。
    /// - `outcome`：本次查询、提交或交付返回的结果。
    ///
    /// # 返回值
    ///
    /// 展示完成返回 `Ok(())`；提交收到 Worker 列表等错误结果类型时返回错误。
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
    /// 应用完成事件，忽略已过期的发现和 Worker 查询结果。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Session` 的可变引用。
    /// - `completion`：已完成的发现、查询或提交事件，包含关联身份及结果。
    ///
    /// # 返回值
    ///
    /// 事件已应用或因版本过期被忽略时返回 `Ok(())`；仍相关的调用或展示失败时返回错误。
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
    /// 在终端在途容量允许时启动动作，并记录最新查询身份。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Session` 的可变引用。
    /// - `action`：尚未执行的终端网络动作。
    /// - `tasks`：当前在途动作集合，接收新任务并负责回收完成结果。
    /// - `execute`：把动作转换为异步完成事件的执行函数。
    ///
    /// # 返回值
    ///
    /// 成功排入任务集合返回 `Ok(())`；达到在途上限时返回错误且不发送。
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
/// 判断 Worker 快照是否处于未过期的 Ready 状态。
///
/// # 参数
///
/// - `worker`：待判断可用性的 Worker 快照。
///
/// # 返回值
///
/// 仅当快照未失联且状态为 Ready 时返回 `true`。
fn available(worker: &WorkerView) -> bool {
    !worker.stale && worker.presence.state == PresenceState::Ready
}
/// 把用户输入的一基编号转换为零基索引。
///
/// # 参数
///
/// - `input`：用户输入的一基十进制编号，必须大于零。
///
/// # 返回值
///
/// 成功返回索引；不是整数或输入为零时返回错误。
fn index(input: &str) -> Result<usize> {
    input
        .parse::<usize>()?
        .checked_sub(1)
        .context("编号从 1 开始")
}

/// 连接消息后端，运行 Console 交互并处理 Ctrl+C。
///
/// # 参数
///
/// - `settings`：调用方提供的配置；本函数按其身份、期限和基础设施设置运行。
///
/// # 返回值
///
/// 正常退出或停止等待返回 `Ok(())`；配置、连接、输入、任务或信号失败时返回错误。
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

/// 并发驱动终端输入和网络动作，EOF 后排空在途动作。
///
/// # 参数
///
/// - `session`：需要更新的交互会话，包括选择、列表和最新查询身份。
/// - `input`：逐行产生文本或 I/O 错误的接收端；通道结束视为 EOF。
/// - `execute`：把每个终端动作转换为可独立等待的完成事件 future。
///
/// # 返回值
///
/// 在途动作排空后返回 `Ok(())`；初始动作、输入或任务管理失败时返回错误。
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

/// 当前模块关键行为与边界条件的单元测试，实现在 tests/unit 目录。
#[cfg(test)]
#[path = "../tests/unit/session.rs"]
mod tests;

/// 启动标准输入读取线程，逐行转发读取结果。
///
/// # 返回值
///
/// 有界行输入接收端；每项是文本或读取错误，EOF 后通道关闭。
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
