use crate::{Key, Local, invalid_deadline};
use beamlet_api::{
    AuthorityEvent, AuthorityInbox, AuthorityInfo, ClientTransport, Command, ConnectionView,
    DiscoveryInbox, DiscoveryReply, Pool, Reply, ReplyError, Responder, TransportError, Uuid,
};
use std::{collections::BTreeSet, time::Duration};
use tokio::sync::{mpsc, oneshot};

/// Authority 的有界控制事件入口, 丢弃时解除全部 pool 绑定.
pub struct AuthorityEndpoint<P = String, R = String> {
    local: Local<P, R>,
    keys: Vec<Key>,
    receive: mpsc::Receiver<AuthorityEvent<P, R>>,
}
impl<P, R> std::fmt::Debug for AuthorityEndpoint<P, R> {
    /// 向格式化目标写入 `AuthorityEndpoint<P, R>` 的调试信息。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `AuthorityEndpoint<P, R>` 的共享引用。
    /// - `f`：接收格式化文本的写入目标。
    ///
    /// # 返回值
    ///
    /// 写入成功返回 `Ok(())`；格式化目标拒绝写入时返回 `fmt::Error`。
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthorityEndpoint")
            .field("keys", &self.keys)
            .finish_non_exhaustive()
    }
}
/// 发现查询的多回复接收端, 容量来自 Local 配置.
#[derive(Debug)]
pub struct Discovery(mpsc::Receiver<DiscoveryReply>);

/// 在本地消息空间为 Authority 的各个 pool 注册控制入口。
///
/// # 参数
///
/// - `local`：已创建的进程内消息空间句柄，克隆共享相同路由表。
/// - `info`：Authority 的角色/会话身份、标签及其加入的 pools。
///
/// # 返回值
///
/// 成功返回 AuthorityEndpoint；身份/pools 非法或已有相同绑定返回 TransportError。
pub(crate) fn bind<P: Send + 'static, R: Send + 'static>(
    local: Local<P, R>,
    info: &AuthorityInfo,
) -> Result<AuthorityEndpoint<P, R>, TransportError> {
    if info.instance.role_id.is_nil() || info.instance.session_id.is_nil() || info.pools.is_empty()
    {
        return Err(TransportError::new("invalid authority binding"));
    }
    let keys: Vec<_> = info
        .pools
        .iter()
        .map(|pool| (pool.clone(), info.instance))
        .collect();
    let (send, receive) = mpsc::channel(local.capacity);
    {
        let mut routes = local.routes.lock().expect("local routes lock poisoned");
        if keys.iter().any(|key| routes.authorities.contains_key(key)) {
            return Err(TransportError::new("authority already bound"));
        }
        for key in &keys {
            routes.authorities.insert(key.clone(), send.clone());
        }
    }
    Ok(AuthorityEndpoint {
        local,
        keys,
        receive,
    })
}
impl<P, R> Drop for AuthorityEndpoint<P, R> {
    /// 释放 Authority 端点拥有的全部本地路由绑定。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `AuthorityEndpoint<P, R>` 的可变引用。
    ///
    /// # 返回值
    ///
    /// `()`；从路由表移除本端点的 pool/实例入口。
    fn drop(&mut self) {
        let mut routes = self
            .local
            .routes
            .lock()
            .expect("local routes lock poisoned");
        for key in &self.keys {
            routes.authorities.remove(key);
        }
    }
}
impl<P: Send + 'static, R: Send + 'static> AuthorityInbox<P, R> for AuthorityEndpoint<P, R> {
    /// 取得能够持续读取当前连接状态的视图。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `AuthorityEndpoint<P, R>` 的共享引用。
    ///
    /// # 返回值
    ///
    /// 连接视图句柄；后续查询反映连接当前世代和状态。
    fn connection(&self) -> ConnectionView {
        self.local.connection()
    }
    /// 从本地控制队列取出一个 Authority 事件。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `AuthorityEndpoint<P, R>` 的可变引用。
    ///
    /// # 返回值
    ///
    /// 有事件返回 `Ok(Some(event))`，队列关闭并排空返回 `Ok(None)`。
    async fn receive(&mut self) -> Result<Option<AuthorityEvent<P, R>>, TransportError> {
        Ok(self.receive.recv().await)
    }
    /// 请求所绑定 pools 中的 Worker 重新发布完整快照。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `AuthorityEndpoint<P, R>` 的共享引用。
    ///
    /// # 返回值
    ///
    /// 发送/通知完成返回 `Ok(())`；传输失败返回错误，不保证 Worker 已回复。
    async fn probe(&self) -> Result<(), TransportError> {
        let routes = self
            .local
            .routes
            .lock()
            .expect("local routes lock poisoned");
        for ((pool, _), route) in &routes.workers {
            if self.keys.iter().any(|(subscribed, _)| subscribed == pool) {
                route
                    .probes
                    .send_modify(|sequence| *sequence = sequence.wrapping_add(1));
            }
        }
        Ok(())
    }
}
impl DiscoveryInbox for Discovery {
    /// 从本地发现通道取出一份回复。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Discovery` 的可变引用。
    ///
    /// # 返回值
    ///
    /// 有回复返回 `Ok(Some(reply))`，所有发送者结束后返回 `Ok(None)`。
    async fn receive(&mut self) -> Result<Option<DiscoveryReply>, TransportError> {
        Ok(self.0.recv().await)
    }
}
impl<P: Send + 'static, R: Send + 'static> ClientTransport<P, R> for Local<P, R> {
    type Discovery = Discovery;
    /// 向本地消息空间内目标 pools 的 Authority 发出发现请求。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Local<P, R>` 的共享引用。
    /// - `pools`：本次发现或登记关注的 pool 集合，按精确名称匹配。
    /// - `request_id`：请求与回复关联 UUID；不作为 Operation 身份或幂等凭据。
    ///
    /// # 返回值
    ///
    /// 返回 Discovery 接收端；发现使用尽力发送，满队列允许丢失。
    async fn discover(
        &self,
        pools: &BTreeSet<Pool>,
        request_id: Uuid,
    ) -> Result<Discovery, TransportError> {
        let (send, receive) = mpsc::channel(self.capacity);
        let routes = self.routes.lock().expect("local routes lock poisoned");
        for ((pool, _), target) in &routes.authorities {
            if !pools.contains(pool) {
                continue;
            }
            let reply = send.clone();
            let _ = target.try_send(AuthorityEvent::Discovery {
                pool: pool.clone(),
                request_id,
                reply: Responder::new(move |value| async move {
                    reply.try_send(value).map_err(ReplyError::new)
                }),
            });
        }
        Ok(Discovery(receive))
    }
    /// 通过本地路由向一个 Authority 会话发出一次请求。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Local<P, R>` 的共享引用。
    /// - `command`：本次 Authority 命令，携带请求 UUID、目标会话及查询/提交数据。
    /// - `deadline`：等待本次回复的最长时长，非绝对时间；超时不取消远端执行。
    ///
    /// # 返回值
    ///
    /// 成功返回 Reply；期限非法、无接收者、队列满/关闭或等待失败返回 TransportError。
    async fn request(
        &self,
        command: Command<P>,
        deadline: Duration,
    ) -> Result<Reply<R>, TransportError> {
        if invalid_deadline(deadline) {
            return Err(TransportError::new("invalid request deadline"));
        }
        let pool = command.pool().clone();
        let sender = self
            .routes
            .lock()
            .expect("local routes lock poisoned")
            .authorities
            .get(&(pool.clone(), command.authority()))
            .cloned()
            .ok_or_else(|| TransportError::new("authority has no receiver"))?;
        let (send, receive) = oneshot::channel();
        sender
            .try_send(AuthorityEvent::Request {
                pool,
                command,
                reply: Responder::new(move |value| async move {
                    send.send(value)
                        .map_err(|_| ReplyError::new("request receiver closed"))
                }),
            })
            .map_err(|_| TransportError::new("authority input full or closed"))?;
        tokio::time::timeout(deadline, receive)
            .await
            .map_err(TransportError::new)?
            .map_err(TransportError::new)
    }
}
