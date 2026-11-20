use crate::{Nats, subject};
use beamlet_api::{
    Connection, Incoming, Presence, PresenceChannel, PresenceEvent, ReplyError, Responder,
    TransportError, WorkerBinding, WorkerInbox, WorkerResponse,
};
use beamlet_protocol::{WorkerCall, WorkerReply, decode, encode};
use futures::StreamExt;

/// 已绑定唯一实例的调用接收端, 不执行任务或维护心跳计时.
#[derive(Debug)]
pub struct WorkerEndpoint {
    /// 本端点共享的 NATS 连接句柄.
    nats: Nats,
    /// Worker 调用许可的固定快照, 包含 pool, 角色会话和启用能力.
    binding: WorkerBinding,
    /// 唯一 Worker 会话调用主题的订阅.
    subscription: async_nats::Subscriber,
}
/// 公告发送与探测/重连接收端, 不改变公告状态或序号.
#[derive(Debug)]
pub struct PresenceEndpoint {
    /// 本端点共享的 NATS 连接句柄.
    nats: Nats,
    /// Worker 或调用所属的唯一 pool.
    pool: beamlet_api::Pool,
    /// 固定角色 UUID 和本次运行会话 UUID 组成的身份.
    instance: beamlet_api::RoleInstance,
    /// Worker 所在 pool 的探测主题订阅.
    probes: async_nats::Subscriber,
}

/// 订阅指定 Worker 会话的调用和探测主题.
///
/// # 参数
///
/// - `nats`: 已连接的 NATS 客户端及连接状态句柄.
/// - `binding`: Worker 的 pool, 角色/会话身份和启用能力集合.
///
/// # 返回值
///
/// 成功返回 `(WorkerEndpoint, PresenceEndpoint)`; 绑定, 订阅或刷新失败返回 TransportError.
pub(crate) async fn bind(
    nats: Nats,
    binding: WorkerBinding,
) -> Result<(WorkerEndpoint, PresenceEndpoint), TransportError> {
    binding.validate()?;
    let subscription = nats
        .client
        .subscribe(subject::worker(&binding.pool, binding.instance))
        .await
        .map_err(TransportError::new)?;
    let probes = nats
        .client
        .subscribe(subject::probe(&binding.pool))
        .await
        .map_err(TransportError::new)?;
    nats.flush().await?;
    let presence = PresenceEndpoint {
        nats: nats.clone(),
        pool: binding.pool.clone(),
        instance: binding.instance,
        probes,
    };
    Ok((
        WorkerEndpoint {
            nats,
            binding,
            subscription,
        },
        presence,
    ))
}
impl WorkerInbox for WorkerEndpoint {
    /// 解码 NATS Worker 调用, 拒绝不匹配的目标后继续接收.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `WorkerEndpoint` 的可变引用.
    ///
    /// # 返回值
    ///
    /// 有效调用为 `Ok(Some(Incoming))`; 订阅结束为 `Ok(None)`; 拒绝回复发送失败返回 TransportError.
    async fn receive(&mut self) -> Result<Option<Incoming>, TransportError> {
        while let Some(message) = self.subscription.next().await {
            let Some(subject) = message.reply else {
                continue;
            };
            let wire: WorkerCall = match decode(&message.payload) {
                Ok(wire) => wire,
                Err(error) => {
                    tracing::warn!(event = "attempt.rejected", %error, "忽略无效调用");
                    continue;
                }
            };
            let call = wire.into();
            if !self.binding.accepts(&call) {
                let rejection = WorkerReply::Rejected {
                    pool: self.binding.pool.clone(),
                    worker: self.binding.instance,
                    operation_id: call.attempt.operation_id().clone(),
                    attempt_id: call.attempt.id().clone(),
                    reason: "目标身份、pool 或能力不匹配".into(),
                };
                self.nats
                    .client
                    .publish(
                        subject,
                        encode(&rejection).map_err(TransportError::new)?.into(),
                    )
                    .await
                    .map_err(TransportError::new)?;
                continue;
            }
            let client = self.nats.client.clone();
            let pool = self.binding.pool.clone();
            let worker = self.binding.instance;
            let operation_id = call.attempt.operation_id().clone();
            let attempt_id = call.attempt.id().clone();
            return Ok(Some(Incoming {
                call,
                reply: Responder::new(move |response: WorkerResponse| async move {
                    let reply = match response {
                        WorkerResponse::Report(report) => WorkerReply::Report {
                            pool,
                            worker,
                            report: report.into(),
                        },
                        WorkerResponse::Failed(reason) => WorkerReply::Failed {
                            pool,
                            worker,
                            operation_id,
                            attempt_id,
                            reason,
                        },
                    };
                    client
                        .publish(subject, encode(&reply).map_err(ReplyError::new)?.into())
                        .await
                        .map_err(ReplyError::new)
                }),
            }));
        }
        Ok(None)
    }
    /// 排空 Worker 的 NATS 订阅并刷新关闭请求.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `WorkerEndpoint` 的可变引用.
    ///
    /// # 返回值
    ///
    /// 完成返回 `Ok(())`; 订阅排空或刷新失败返回 TransportError, 已接收缓冲仍由 Worker 消费.
    async fn stop(&mut self) -> Result<(), TransportError> {
        self.subscription
            .drain()
            .await
            .map_err(TransportError::new)?;
        self.nats.flush().await
    }
}
impl PresenceChannel for PresenceEndpoint {
    /// 读取公告通道的当前连接状态.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `PresenceEndpoint` 的共享引用.
    ///
    /// # 返回值
    ///
    /// 包含观测世代与是否已连接的 Connection 快照.
    fn connection(&self) -> Connection {
        *self.nats.status.borrow()
    }
    /// 等待 Worker 探测请求或连接状态变化.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `PresenceEndpoint` 的可变引用.
    ///
    /// # 返回值
    ///
    /// 成功返回 Probe 或 Connection 事件; 观察入口关闭或传输失败返回错误.
    async fn receive(&mut self) -> Result<PresenceEvent, TransportError> {
        tokio::select! {
            biased;
            changed = self.nats.status.changed() => {
                changed.map_err(TransportError::new)?;
                Ok(PresenceEvent::Connection(*self.nats.status.borrow_and_update()))
            }
            probe = self.probes.next() => {
                probe.ok_or_else(|| TransportError::new("worker probe subscription closed"))?;
                Ok(PresenceEvent::Probe)
            }
        }
    }
    /// 校验公告与端点绑定一致后编码并发送到 NATS.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `PresenceEndpoint` 的共享引用.
    /// - `presence`: Worker 的完整状态快照, 包括 pool, 角色会话, 能力, 序号和运行状态.
    ///
    /// # 返回值
    ///
    /// 传输接受公告返回 `Ok(())`; 身份不匹配, 编码或发布失败返回 TransportError.
    async fn publish(&self, presence: &Presence) -> Result<(), TransportError> {
        if presence.pool != self.pool || presence.instance != self.instance {
            return Err(TransportError::new("presence does not match binding"));
        }
        self.nats
            .client
            .publish(
                subject::presence(&self.pool),
                encode(presence).map_err(TransportError::new)?.into(),
            )
            .await
            .map_err(TransportError::new)
    }
    /// 等待传输缓冲完成刷新.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `PresenceEndpoint` 的共享引用.
    ///
    /// # 返回值
    ///
    /// 刷新完成返回 `Ok(())`; 失败返回 TransportError, 成功不代表远端已接受业务.
    async fn flush(&self) -> Result<(), TransportError> {
        self.nats.flush().await
    }
}
