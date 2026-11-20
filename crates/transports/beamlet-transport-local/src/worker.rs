use crate::{Key, Local, WorkerRoute, invalid_deadline};
use beamlet_api::{
    AuthorityEvent, Connection, Delivery, DeliveryError, Incoming, Presence, PresenceChannel,
    PresenceEvent, ReplyError, Responder, TransportError, WorkerBinding, WorkerInbox, WorkerReport,
    WorkerResponse, validation,
};
use std::time::Duration;
use tokio::{
    sync::{mpsc, oneshot, watch},
    time::timeout,
};

/// 有界调用接收端, 正常 stop 后仍可排空已接收队列.
///
/// # 泛型参数
///
/// - `P`: 业务输入类型, 默认 String.
/// - `R`: 业务结果类型, 默认 String.
pub struct WorkerEndpoint<P = String, R = String> {
    /// 本地消息空间句柄, 端点与同空间其他角色共享路由表.
    local: Local<P, R>,
    /// 本端点唯一绑定的 pool 和角色会话.
    key: Key,
    /// 已经交给该 Worker 的有界调用队列.
    receiver: mpsc::Receiver<Incoming<P, R>>,
}
/// Worker 的完整公告发送端与合并后的探测通知入口.
///
/// # 泛型参数
///
/// - `P`: 业务输入类型, 默认 String.
/// - `R`: 业务结果类型, 默认 String.
pub struct PresenceEndpoint<P = String, R = String> {
    /// 本地消息空间句柄, 端点与同空间其他角色共享路由表.
    local: Local<P, R>,
    /// 本端点唯一绑定的 pool 和角色会话.
    key: Key,
    /// 合并后的探测序号接收端, 变化时要求 Worker 重新公告.
    probes: watch::Receiver<u64>,
}
impl<P, R> std::fmt::Debug for WorkerEndpoint<P, R> {
    /// 向格式化目标写入 `WorkerEndpoint<P, R>` 的调试信息.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `WorkerEndpoint<P, R>` 的共享引用.
    /// - `f`: 接收格式化文本的写入目标.
    ///
    /// # 返回值
    ///
    /// 写入成功返回 `Ok(())`; 格式化目标拒绝写入时返回 `fmt::Error`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkerEndpoint")
            .field("key", &self.key)
            .finish_non_exhaustive()
    }
}
impl<P, R> std::fmt::Debug for PresenceEndpoint<P, R> {
    /// 向格式化目标写入 `PresenceEndpoint<P, R>` 的调试信息.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `PresenceEndpoint<P, R>` 的共享引用.
    /// - `f`: 接收格式化文本的写入目标.
    ///
    /// # 返回值
    ///
    /// 写入成功返回 `Ok(())`; 格式化目标拒绝写入时返回 `fmt::Error`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PresenceEndpoint")
            .field("key", &self.key)
            .finish_non_exhaustive()
    }
}
/// 注册本地 Worker 调用入口和探测/公告端点.
///
/// # 参数
///
/// - `local`: 已创建的进程内消息空间句柄, 克隆共享相同路由表.
/// - `binding`: Worker 的 pool, 角色/会话身份和启用能力集合.
///
/// # 返回值
///
/// 成功返回 `(WorkerEndpoint, PresenceEndpoint)`; 绑定非法或重复返回 TransportError.
pub(crate) fn bind<P: Send + 'static, R: Send + 'static>(
    local: Local<P, R>,
    binding: WorkerBinding,
) -> Result<crate::WorkerEndpoints<P, R>, TransportError> {
    binding.validate()?;
    let key = (binding.pool.clone(), binding.instance);
    let (sender, receiver) = mpsc::channel(local.capacity);
    let (probes, receive_probes) = watch::channel(0);
    {
        let mut routes = local.routes.lock().expect("local routes lock poisoned");
        if routes.workers.contains_key(&key) {
            return Err(TransportError::new("worker already bound"));
        }
        routes.workers.insert(
            key.clone(),
            WorkerRoute {
                binding,
                sender,
                probes,
            },
        );
    }
    Ok((
        WorkerEndpoint {
            local: local.clone(),
            key: key.clone(),
            receiver,
        },
        PresenceEndpoint {
            local,
            key,
            probes: receive_probes,
        },
    ))
}
impl<P, R> Drop for WorkerEndpoint<P, R> {
    /// 丢弃 Worker 端点并移除它的本地调用路由.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `WorkerEndpoint<P, R>` 的可变引用.
    ///
    /// # 返回值
    ///
    /// `()`; 释放该绑定, 允许后续重新绑定.
    fn drop(&mut self) {
        self.local
            .routes
            .lock()
            .expect("local routes lock poisoned")
            .workers
            .remove(&self.key);
    }
}
impl<P: Send + 'static, R: Send + 'static> WorkerInbox<P, R> for WorkerEndpoint<P, R> {
    /// 接收本地 Worker 调用并保留回复入口.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `WorkerEndpoint<P, R>` 的可变引用.
    ///
    /// # 返回值
    ///
    /// 有调用返回 `Ok(Some(Incoming))`; 关闭且排空后返回 `Ok(None)`.
    async fn receive(&mut self) -> Result<Option<Incoming<P, R>>, TransportError> {
        Ok(self.receiver.recv().await)
    }
    /// 移除 Worker 路由并关闭新输入, 保留已接收调用.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `WorkerEndpoint<P, R>` 的可变引用.
    ///
    /// # 返回值
    ///
    /// 返回 `Ok(())`; 缓冲内调用仍可通过 receive 排空.
    async fn stop(&mut self) -> Result<(), TransportError> {
        self.receiver.close();
        Ok(())
    }
}
impl<P: Send + 'static, R: Send + 'static> Delivery<P, R> for Local<P, R> {
    /// 通过本地路由向唯一 Worker 交付调用并等待报告.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Local<P, R>` 的共享引用.
    /// - `call`: 已指定 pool, Authority 和 Worker 会话的调用, 包含 Attempt, 能力及业务输入.
    /// - `deadline`: 等待本次回复的最长时长, 非绝对时间; 超时不取消远端执行.
    ///
    /// # 返回值
    ///
    /// 成功返回经关联校验的报告; 无接收者, 明确拒收或等待结果未知返回 DeliveryError.
    async fn deliver(
        &self,
        call: beamlet_api::Call<P>,
        deadline: Duration,
    ) -> Result<WorkerReport<R>, DeliveryError> {
        validation::check_call(&call)?;
        if invalid_deadline(deadline) {
            return Err(DeliveryError::NotDelivered(
                "invalid report deadline".into(),
            ));
        }
        let sender = {
            let routes = self.routes.lock().expect("local routes lock poisoned");
            let route = routes
                .workers
                .get(&(call.pool.clone(), call.worker))
                .ok_or(DeliveryError::NoReceiver)?;
            if route.sender.is_closed() {
                return Err(DeliveryError::NoReceiver);
            }
            if !route.binding.accepts(&call) {
                return Err(DeliveryError::NotDelivered(
                    "目标身份、pool 或能力不匹配".into(),
                ));
            }
            route.sender.clone()
        };
        let operation_id = call.attempt.operation_id().clone();
        let attempt_id = call.attempt.id().clone();
        let (send, receive) = oneshot::channel::<WorkerResponse<R>>();
        sender
            .try_send(Incoming {
                call,
                reply: Responder::new(move |response| async move {
                    send.send(response)
                        .map_err(|_| ReplyError::new("报告接收端已关闭"))
                }),
            })
            .map_err(|error| match error {
                mpsc::error::TrySendError::Closed(_) => DeliveryError::NoReceiver,
                mpsc::error::TrySendError::Full(_) => {
                    DeliveryError::NotDelivered("接收队列已满".into())
                }
            })?;
        let response = timeout(deadline, receive)
            .await
            .map_err(|_| DeliveryError::Unknown("等待报告超时".into()))?
            .map_err(|_| DeliveryError::Unknown("接收者未返回报告".into()))?;
        match response {
            WorkerResponse::Report(report) => {
                validation::check_report(report, &operation_id, &attempt_id)
            }
            WorkerResponse::Failed(reason) => Err(DeliveryError::Unknown(reason)),
        }
    }
}
impl<P: Send + 'static, R: Send + 'static> PresenceChannel for PresenceEndpoint<P, R> {
    /// 读取公告通道的当前连接状态.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `PresenceEndpoint<P, R>` 的共享引用.
    ///
    /// # 返回值
    ///
    /// 包含观测世代与是否已连接的 Connection 快照.
    fn connection(&self) -> Connection {
        self.local.connection().current()
    }
    /// 等待本地 Authority 发出的 Worker 探测通知.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `PresenceEndpoint<P, R>` 的可变引用.
    ///
    /// # 返回值
    ///
    /// 收到通知返回 `Ok(PresenceEvent::Probe)`; 探测通道关闭返回 TransportError.
    async fn receive(&mut self) -> Result<PresenceEvent, TransportError> {
        self.probes.changed().await.map_err(TransportError::new)?;
        self.probes.borrow_and_update();
        Ok(PresenceEvent::Probe)
    }
    /// 向本地 Authority 尽力广播完整 Worker 公告.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `PresenceEndpoint<P, R>` 的共享引用.
    /// - `presence`: Worker 的完整状态快照, 包括 pool, 角色会话, 能力, 序号和运行状态.
    ///
    /// # 返回值
    ///
    /// 身份与绑定匹配时返回 `Ok(())`, 不匹配返回 TransportError; 满队列允许丢失.
    async fn publish(&self, presence: &Presence) -> Result<(), TransportError> {
        if presence.pool != self.key.0 || presence.instance != self.key.1 {
            return Err(TransportError::new("presence does not match binding"));
        }
        let routes = self
            .local
            .routes
            .lock()
            .expect("local routes lock poisoned");
        // 公告是可丢失的完整快照, 不使心跳积压阻塞其他 Authority.
        for ((pool, _), sender) in &routes.authorities {
            if pool == &presence.pool {
                let _ = sender.try_send(AuthorityEvent::Presence {
                    presence: presence.clone(),
                    epoch: 1,
                });
            }
        }
        Ok(())
    }
    /// 完成本地传输刷新; 本地通道不需要额外网络刷新.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `PresenceEndpoint<P, R>` 的共享引用.
    ///
    /// # 返回值
    ///
    /// 始终返回 `Ok(())`.
    async fn flush(&self) -> Result<(), TransportError> {
        Ok(())
    }
}
