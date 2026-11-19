use crate::{
    LocalInbox, Request, RunError, Transports,
    engine::{Ingress, Reply},
    registry::{Registry, Source},
    transports::Deliveries,
};
use beamlet_api::{
    AuthorityEvent, AuthorityInbox, AuthorityInfo, Command, DiscoveryReply, Outcome, Responder,
};
use std::time::Instant;
use tokio::{sync::mpsc, task::JoinSet};
use tracing::Instrument;

/// 合并 Authority 本地与远端提交的入口, 并拥有独立的控制处理任务.
///
/// # 泛型参数
///
/// - `P`: 业务输入类型.
/// - `R`: 业务结果类型.
/// - `L`: 本地输入类型.
pub(crate) struct Inputs<P, R, L> {
    /// 独立控制任务分类后交给业务编排的有界远端提交队列.
    remote: mpsc::Receiver<Request<P, R>>,
    /// 可选本地业务输入来源.
    local: L,
    /// 本地输入是否已返回永久 EOF, 避免继续轮询它.
    local_closed: bool,
    /// 组合入口是否已停止, 不再交出新请求.
    stopped: bool,
    /// 各传输的控制任务; 丢弃 Inputs 时全部中止.
    control: JoinSet<Result<(), RunError>>,
}
impl<P: Send + 'static, R: Send + 'static, L: LocalInbox<P, R>> Inputs<P, R, L> {
    /// 运行 transports 的控制入口, 将 info/registry 与 local 合并到容量为 capacity 的队列.
    /// 返回输入与按路由排序的交付出口; 空传输集合返回错误.
    pub(crate) fn connect(
        transports: Transports<P, R>,
        info: AuthorityInfo,
        registry: Registry,
        local: L,
        capacity: usize,
        max_concurrent_replies: usize,
    ) -> Result<(Self, Deliveries<P, R>), RunError> {
        let (send, remote) = mpsc::channel(capacity);
        let mut control = JoinSet::new();
        let deliveries =
            transports.start(info, registry, send, max_concurrent_replies, &mut control)?;
        Ok((
            Self {
                remote,
                local,
                local_closed: false,
                stopped: false,
                control,
            },
            deliveries,
        ))
    }
}
impl<P: Send + 'static, R: Send + 'static, L: LocalInbox<P, R>> Ingress<P, R> for Inputs<P, R, L> {
    /// 合并本地和远端业务请求, 检测控制任务异常结束.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Inputs<P, R, L>` 的可变引用.
    ///
    /// # 返回值
    ///
    /// 收到提交返回 `Ok(Some(Request))`; 已停止返回 `Ok(None)`; 输入或控制任务失败返回 RunError.
    async fn receive(&mut self) -> Result<Option<Request<P, R>>, RunError> {
        while !self.stopped {
            tokio::select! {
                result = self.control.join_next() => {
                    if let Some(result) = result {
                        result.map_err(|error| RunError::Task(beamlet_api::TransportError::new(error)))??;
                    }
                    return Err(RunError::Invariant("Authority 控制任务意外结束"));
                }
                request = self.remote.recv() => {
                    if let Some(request) = request { return Ok(Some(request)); }
                    continue;
                }
                request = self.local.receive(), if !self.local_closed => {
                    match request? { Some(request) => return Ok(Some(request)), None => self.local_closed = true }
                }
            }
        }
        Ok(None)
    }
    /// 停止 Authority 控制接收并丢弃未签发的排队请求.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Inputs<P, R, L>` 的可变引用.
    ///
    /// # 返回值
    ///
    /// 停止本地输入成功返回 `Ok(())`; 本地停止失败返回 RunError.
    async fn stop(&mut self) -> Result<(), RunError> {
        self.stopped = true;
        self.control.abort_all();
        self.remote.close();
        while self.remote.try_recv().is_ok() {}
        self.local.stop().await?;
        Ok(())
    }
}
/// 异步发送控制回复, 并在队列满或发送失败时记录日志而不阻塞控制循环.
fn spawn_reply<T: Send + 'static>(
    replies: &mut JoinSet<()>,
    limit: usize,
    reply: Responder<T>,
    value: T,
    kind: &'static str,
) {
    while let Some(finished) = replies.try_join_next() {
        if let Err(error) = finished {
            tracing::warn!(event = "control.reply_task_failed", %error, "控制回复子任务异常结束");
        }
    }
    if replies.len() >= limit {
        tracing::warn!(
            event = "control.reply_dropped",
            kind,
            limit,
            "控制回复在途已满, 丢弃本次回复"
        );
        return;
    }
    replies.spawn(
        async move {
            if let Err(error) = reply.send(value).await {
                tracing::warn!(event = "control.reply_failed", kind, %error, "发送控制回复失败");
            }
        }
        .in_current_span(),
    );
}

/// 独立处理发现, Worker 公告和查询, 将提交分类到有界队列.
///
/// # 参数
///
/// - `endpoint`: 属于当前 Authority 身份和 pools 的控制事件接收端.
/// - `info`: Authority 的角色/会话身份, 标签及其加入的 pools.
/// - `registry`: 当前 Authority 独立维护的 Worker 登记视图.
/// - `source`: 此端点的稳定路由和连接视图.
/// - `send`: 把已分类的远端提交交给业务循环的有界发送端; 满时明确拒绝.
/// - `max_concurrent_replies`: 控制回复在途任务的并发上限.
///
/// # 返回值
///
/// 持续运行直到输入关闭或处理出错并返回 RunError; 单次回复或探测失败仅记录日志; 任务通常由上层中止.
pub(crate) async fn control<P: Send + 'static, R: Send + 'static>(
    mut endpoint: impl AuthorityInbox<P, R>,
    info: AuthorityInfo,
    registry: Registry,
    source: Source,
    send: mpsc::Sender<Request<P, R>>,
    max_concurrent_replies: usize,
) -> Result<(), RunError> {
    if let Err(error) = endpoint.probe().await {
        tracing::warn!(
            event = "authority.probe_failed",
            %error,
            "启动时发送探测失败, 继续运行"
        );
    }
    let mut cleanup = tokio::time::interval(std::time::Duration::from_secs(1));
    cleanup.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut replies = JoinSet::new();
    loop {
        let event = tokio::select! {
            biased;
            _ = cleanup.tick() => { registry.expire(Instant::now()); continue; }
            finished = replies.join_next(), if !replies.is_empty() => {
                finished
                    .expect("nonempty control reply task set")
                    .map_err(|error| RunError::Task(beamlet_api::TransportError::new(error)))?;
                continue;
            }
            event = endpoint.receive() => event?.ok_or(RunError::Invariant("Authority 订阅已关闭"))?,
        };
        match event {
            AuthorityEvent::Connection(link) => {
                if link.connected
                    && let Err(error) = endpoint.probe().await
                {
                    tracing::warn!(
                        event = "authority.probe_failed",
                        %error,
                        "连接恢复后发送探测失败, 继续运行"
                    );
                }
            }
            AuthorityEvent::Presence { presence, epoch } => {
                registry.observe(&source, presence, epoch, Instant::now());
            }
            AuthorityEvent::Discovery {
                request_id, reply, ..
            } => {
                spawn_reply(
                    &mut replies,
                    max_concurrent_replies,
                    reply,
                    DiscoveryReply {
                        request_id,
                        authority: info.clone(),
                    },
                    "discovery",
                );
            }
            AuthorityEvent::Request {
                pool,
                command,
                reply,
            } => {
                let request_id = command.request_id();
                let authority = info.instance;
                let reply = Reply::new(move |outcome| async move {
                    reply
                        .send(beamlet_api::Reply {
                            request_id,
                            authority,
                            outcome,
                        })
                        .await
                });
                if command.authority() != info.instance
                    || command.pool() != &pool
                    || request_id.is_nil()
                    || !info.pools.contains(&pool)
                {
                    spawn_reply(
                        &mut replies,
                        max_concurrent_replies,
                        reply,
                        Outcome::Rejected {
                            reason: "Authority 会话或 pool 不匹配".into(),
                        },
                        "rejected",
                    );
                    continue;
                }
                match command {
                    Command::Workers { capability, .. } => {
                        let workers =
                            registry.workers(&pool, capability.as_deref(), Instant::now());
                        spawn_reply(
                            &mut replies,
                            max_concurrent_replies,
                            reply,
                            Outcome::Workers { workers },
                            "workers",
                        );
                    }
                    Command::Submit { submission, .. } => {
                        if let Err(error) = send.try_send(Request { submission, reply }) {
                            spawn_reply(
                                &mut replies,
                                max_concurrent_replies,
                                error.into_inner().reply,
                                Outcome::Rejected {
                                    reason: "Authority 接收队列已满或正在停止".into(),
                                },
                                "rejected",
                            );
                        }
                    }
                }
            }
        }
    }
}
