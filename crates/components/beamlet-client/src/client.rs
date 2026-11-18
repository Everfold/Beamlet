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
///
/// # 泛型参数
///
/// - `T`: 客户端传输类型.
/// - `P`: 业务输入类型, 默认 String.
/// - `R`: 业务结果类型, 默认 String.
pub struct Client<T, P = String, R = String> {
    /// 调用方注入的传输句柄, Client 本身不负责连接.
    transport: T,
    /// 构造时已校验的请求总期限和发现窗口.
    options: Options,
    /// 记录输入/结果类型而不实际拥有数据, 也不额外要求它们实现 Clone 或 Sync.
    data: PhantomData<fn(P) -> R>,
}
impl<T: Clone, P, R> Clone for Client<T, P, R> {
    /// 复制 Client 参数和传输句柄, 保留调用之间的独立性.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Client<T, P, R>` 的共享引用.
    ///
    /// # 返回值
    ///
    /// 新的 Client; 不克隆业务输入, 结果或在途调用.
    fn clone(&self) -> Self {
        Self {
            transport: self.transport.clone(),
            options: self.options,
            data: PhantomData,
        }
    }
}
impl<T, P, R> std::fmt::Debug for Client<T, P, R> {
    /// 向格式化目标写入 `Client<T, P, R>` 的调试信息.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Client<T, P, R>` 的共享引用.
    /// - `f`: 接收格式化文本的写入目标.
    ///
    /// # 返回值
    ///
    /// 写入成功返回 `Ok(())`; 格式化目标拒绝写入时返回 `fmt::Error`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("options", &self.options)
            .finish_non_exhaustive()
    }
}
impl<T: ClientTransport<P, R>, P: Send, R: Send> Client<T, P, R> {
    /// 校验运行期限并装配 Client, 不发起网络通信.
    ///
    /// # 参数
    ///
    /// - `transport`: 调用方已装配的传输端点; 客户端不创建网络连接.
    /// - `options`: 当前模块的运行参数; 期限使用 Duration, 容量/并发由对应字段约束.
    ///
    /// # 返回值
    ///
    /// 合法返回 Client; 请求期限或发现窗口非法时返回 BuildError.
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
    /// 收集 Authority 发现回复, 按角色/会话去重并合并 pools.
    ///
    /// 每次调用分配独立请求 UUID. 忽略无关/无效回复; 窗口结束返回已收集结果,
    /// 端点 EOF 可提前结束. 总期限包含端点创建; 超时不返回不完整结果.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Client<T, P, R>` 的共享引用.
    /// - `pools`: 本次发现或登记关注的 pool 集合, 按精确名称匹配.
    ///
    /// # 返回值
    ///
    /// 成功返回 Authority 列表; 空发现范围, 传输失败或总期限耗尽返回 Error.
    ///
    /// # Errors
    /// pools 为空, 传输失败或总等待期限耗尽时返回错误.
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
    /// 查询指定 Authority 在一个 pool 中的 Worker 快照.
    ///
    /// 包含 stale/offline 记录; 是否可选由 Authority 在提交时决定.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Client<T, P, R>` 的共享引用.
    /// - `authority`: 明确选定的 Authority 角色与会话 UUID.
    /// - `pool`: 查询, 候选选择或主题构造的目标 pool.
    /// - `capability`: 可选能力过滤条件; None 返回该 pool 中所有能力的记录.
    ///
    /// # 返回值
    ///
    /// 成功返回 WorkerView 列表; 参数, 传输, 期限, 关联错误或明确拒绝返回 Error.
    ///
    /// # Errors
    /// 参数无效, 传输失败, 超时, 回复不匹配或 Authority 拒绝查询时返回错误.
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
    /// 向指定 Authority 提交一次任务, 自动生成请求 UUID.
    ///
    /// Rejected 表示创建前拒绝; Unresolved 表示已经签发但未取得有效 Report.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Client<T, P, R>` 的共享引用.
    /// - `authority`: 明确选定的 Authority 角色与会话 UUID.
    /// - `submission`: 请求的 pool, Worker 选择约束及业务任务; 消费输入数据的所有权.
    ///
    /// # 返回值
    ///
    /// 返回 Completed, Rejected 或 Unresolved 业务结果; 本地/传输/期限/关联失败返回 Error.
    ///
    /// # Errors
    /// 参数无效, 传输失败, 超时或回复不匹配时返回错误, 不换人或重试.
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
    /// 按调用方提供的请求 UUID 发送一次命令并验证回复.
    ///
    /// 消费业务数据且只交给传输一次. UUID 仅用于回复关联, 不提供幂等或去重.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Client<T, P, R>` 的共享引用.
    /// - `command`: 本次 Authority 命令, 携带请求 UUID, 目标会话及查询/提交数据.
    ///
    /// # 返回值
    ///
    /// 成功返回与命令相符的 Outcome; 参数, 传输, 超时或回复不匹配返回 Error, 不重发.
    ///
    /// # Errors
    /// 参数无效, 传输失败, 超时, 请求/会话关联错误或结果种类不匹配时返回错误.
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
