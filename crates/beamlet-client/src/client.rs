use crate::{
    BuildError, Error, Options,
    request::{Expected, valid_instance},
};
use beamlet_api::{
    AuthorityInfo, ClientTransport, Command, DiscoveryInbox, Outcome, Pool, RoleInstance,
    Submission, Uuid, WorkerView,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    marker::PhantomData,
};
use tokio::time::{Instant, timeout, timeout_at};

/// 包装一个传输端点; 克隆仅复制端点句柄和参数, 多个调用独立等待.
pub struct Client<T, P = String, R = String> {
    transport: T,
    options: Options,
    data: PhantomData<fn(P) -> R>,
}
impl<T: Clone, P, R> Clone for Client<T, P, R> {
    fn clone(&self) -> Self {
        Self {
            transport: self.transport.clone(),
            options: self.options,
            data: PhantomData,
        }
    }
}
impl<T, P, R> std::fmt::Debug for Client<T, P, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("options", &self.options)
            .finish_non_exhaustive()
    }
}
impl<T: ClientTransport<P, R>, P: Send, R: Send> Client<T, P, R> {
    /// 注入端点并校验期限, 不进行网络通信或创建任务.
    ///
    /// # Errors
    /// 期限无效或发现窗口不小于请求期限时返回错误.
    pub fn new(transport: T, options: Options) -> Result<Self, BuildError> {
        options.validate()?;
        Ok(Self {
            transport,
            options,
            data: PhantomData,
        })
    }
    /// 发现目标 pools 的 Authority, 按角色与会话去重并合并其声明的 pools.
    ///
    /// 每次调用分配独立请求 UUID. 忽略无关/无效回复; 窗口结束返回已收集结果,
    /// 端点 EOF 可提前结束. 总期限包含端点创建; 超时不返回不完整结果.
    ///
    /// # Errors
    /// pools 为空、传输失败或总等待期限耗尽时返回错误.
    pub async fn discover(&self, pools: &BTreeSet<Pool>) -> Result<Vec<AuthorityInfo>, Error> {
        if pools.is_empty() {
            return Err(Error::InvalidRequest("discovery pools are empty"));
        }
        timeout(self.options.request_timeout, async {
            let request_id = Uuid::new_v4();
            let mut replies = self.transport.discover(pools, request_id).await?;
            let deadline = Instant::now() + self.options.discovery_window;
            let mut authorities: BTreeMap<RoleInstance, AuthorityInfo> = BTreeMap::new();
            while Instant::now() < deadline {
                // 外部端点可能连续立即返回; 主动让出执行, 保证总期限和取消可推进.
                tokio::task::yield_now().await;
                let Ok(response) = timeout_at(deadline, replies.receive()).await else {
                    break;
                };
                let Some(reply) = response? else {
                    break;
                };
                if reply.request_id != request_id
                    || !valid_instance(reply.authority.instance)
                    || reply.authority.pools.is_disjoint(pools)
                {
                    continue;
                }
                authorities
                    .entry(reply.authority.instance)
                    .and_modify(|info| info.pools.extend(reply.authority.pools.clone()))
                    .or_insert(reply.authority);
            }
            Ok(authorities.into_values().collect())
        })
        .await
        .map_err(|_| Error::Timeout)?
    }
    /// 查询一个 Authority 在指定 pool 的 Worker 快照, 可按能力过滤.
    ///
    /// 包含 stale/offline 记录; 是否可选由 Authority 在提交时决定.
    ///
    /// # Errors
    /// 参数无效、传输失败、超时、回复不匹配或 Authority 拒绝查询时返回错误.
    pub async fn workers(
        &self,
        authority: RoleInstance,
        pool: Pool,
        capability: Option<&str>,
    ) -> Result<Vec<WorkerView>, Error> {
        match self
            .request(Command::Workers {
                request_id: Uuid::new_v4(),
                authority,
                pool,
                capability: capability.map(str::to_owned),
            })
            .await?
        {
            Outcome::Workers { workers } => Ok(workers),
            Outcome::Rejected { reason } => Err(Error::Rejected { reason }),
            _ => Err(Error::InvalidReply),
        }
    }
    /// 向指定 Authority 提交一次, 自动分配请求 UUID 并保留完整业务结果.
    ///
    /// Rejected 表示创建前拒绝; Unresolved 表示已经签发但未取得有效 Report.
    ///
    /// # Errors
    /// 参数无效、传输失败、超时或回复不匹配时返回错误, 不换人或重试.
    pub async fn submit(
        &self,
        authority: RoleInstance,
        submission: Submission<P>,
    ) -> Result<Outcome<R>, Error> {
        self.request(Command::Submit {
            request_id: Uuid::new_v4(),
            authority,
            submission,
        })
        .await
    }
    /// 使用调用方的请求 UUID 发送命令, 便于日志及外部交互关联.
    ///
    /// 消费业务数据且只交给传输一次. UUID 仅用于回复关联, 不提供幂等或去重.
    ///
    /// # Errors
    /// 参数无效、传输失败、超时、请求/会话关联错误或结果种类不匹配时返回错误.
    pub async fn request(&self, command: Command<P>) -> Result<Outcome<R>, Error> {
        let expected = Expected::for_command(&command)?;
        let request_id = command.request_id();
        let authority = command.authority();
        tracing::info!(event = "request.send_started", %request_id, "发送请求");
        let reply = timeout(
            self.options.request_timeout,
            self.transport
                .request(command, self.options.request_timeout),
        )
        .await
        .map_err(|_| Error::Timeout)??;
        if reply.request_id != request_id || reply.authority != authority {
            return Err(Error::InvalidReply);
        }
        expected.check(&reply.outcome)?;
        Ok(reply.outcome)
    }
}
