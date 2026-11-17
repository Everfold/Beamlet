use crate::{
    LocalInbox, Request, RunError,
    engine::{Ingress, Reply},
    registry::Registry,
};
use beamlet_api::{
    AuthorityEvent, AuthorityInbox, AuthorityInfo, Command, DiscoveryReply, Outcome,
};
use std::time::Instant;
use tokio::{sync::mpsc, task::JoinHandle};
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
    /// 独立控制处理任务; Inputs 被丢弃时请求中止它.
    control: JoinHandle<Result<(), RunError>>,
}
impl<P: Send + 'static, R: Send + 'static, L: LocalInbox<P, R>> Inputs<P, R, L> {
    /// 探测现有 Worker, 并创建独立控制任务与业务输入队列.
    ///
    /// # 参数
    ///
    /// - `endpoint`: 属于当前 Authority 身份和 pools 的控制事件接收端.
    /// - `info`: Authority 的角色/会话身份, 标签及其加入的 pools.
    /// - `registry`: 当前 Authority 独立维护的 Worker 登记视图.
    /// - `local`: 本地输入来源, 提供尚未签发授权的业务提交.
    /// - `capacity`: 远端业务提交队列的容量, 由已验证的 Authority Options 提供.
    ///
    /// # 返回值
    ///
    /// 成功返回合并本地/远端输入的 Inputs; 初始探测失败返回 RunError.
    pub(crate) async fn connect(
        endpoint: impl AuthorityInbox<P, R> + 'static,
        info: AuthorityInfo,
        registry: Registry,
        local: L,
        capacity: usize,
    ) -> Result<Self, RunError> {
        endpoint.probe().await?;
        let (send, remote) = mpsc::channel(capacity);
        let control = tokio::spawn(control(endpoint, info, registry, send).in_current_span());
        Ok(Self {
            remote,
            local,
            local_closed: false,
            stopped: false,
            control,
        })
    }
}
impl<P, R, L> Drop for Inputs<P, R, L> {
    /// 丢弃输入组合时中止所属控制任务.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Inputs<P, R, L>` 的可变引用.
    ///
    /// # 返回值
    ///
    /// `()`; 向控制任务发出中止请求, 不阻塞等待.
    fn drop(&mut self) {
        self.control.abort();
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
                request = self.remote.recv() => {
                    if let Some(request) = request { return Ok(Some(request)); }
                    (&mut self.control).await.map_err(|error| RunError::Task(beamlet_api::TransportError::new(error)))??;
                    return Err(RunError::Invariant("Authority 控制任务意外结束"));
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
        self.control.abort();
        self.remote.close();
        while self.remote.try_recv().is_ok() {}
        self.local.stop().await?;
        Ok(())
    }
}
/// 独立处理发现, Worker 公告和查询, 将提交分类到有界队列.
///
/// # 参数
///
/// - `endpoint`: 属于当前 Authority 身份和 pools 的控制事件接收端.
/// - `info`: Authority 的角色/会话身份, 标签及其加入的 pools.
/// - `registry`: 当前 Authority 独立维护的 Worker 登记视图.
/// - `send`: 把已分类的远端提交交给业务循环的有界发送端; 满时明确拒绝.
///
/// # 返回值
///
/// 持续运行直到输入关闭或处理出错并返回 RunError; 任务通常由上层中止.
async fn control<P: Send + 'static, R: Send + 'static>(
    mut endpoint: impl AuthorityInbox<P, R>,
    info: AuthorityInfo,
    registry: Registry,
    send: mpsc::Sender<Request<P, R>>,
) -> Result<(), RunError> {
    let mut cleanup = tokio::time::interval(std::time::Duration::from_secs(1));
    cleanup.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        let event = tokio::select! {
            _ = cleanup.tick() => { registry.expire(Instant::now()); continue; }
            event = endpoint.receive() => event?.ok_or(RunError::Invariant("Authority 订阅已关闭"))?,
        };
        match event {
            AuthorityEvent::Connection(link) => {
                if link.connected {
                    endpoint.probe().await?;
                }
            }
            AuthorityEvent::Presence { presence, epoch } => {
                registry.observe(presence, epoch, Instant::now());
            }
            AuthorityEvent::Discovery {
                request_id, reply, ..
            } => {
                reply
                    .send(DiscoveryReply {
                        request_id,
                        authority: info.clone(),
                    })
                    .await?;
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
                    reply
                        .send(Outcome::Rejected {
                            reason: "Authority 会话或 pool 不匹配".into(),
                        })
                        .await?;
                    continue;
                }
                match command {
                    Command::Workers { capability, .. } => {
                        reply
                            .send(Outcome::Workers {
                                workers: registry.workers(
                                    &pool,
                                    capability.as_deref(),
                                    Instant::now(),
                                ),
                            })
                            .await?;
                    }
                    Command::Submit { submission, .. } => {
                        if let Err(error) = send.try_send(Request { submission, reply }) {
                            error
                                .into_inner()
                                .reply
                                .send(Outcome::Rejected {
                                    reason: "Authority 接收队列已满或正在停止".into(),
                                })
                                .await?;
                        }
                    }
                }
            }
        }
    }
}
