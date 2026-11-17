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
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthorityEndpoint")
            .field("keys", &self.keys)
            .finish_non_exhaustive()
    }
}
/// 发现查询的多回复接收端, 容量来自 Local 配置.
#[derive(Debug)]
pub struct Discovery(mpsc::Receiver<DiscoveryReply>);

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
    fn connection(&self) -> ConnectionView {
        self.local.connection()
    }
    async fn receive(&mut self) -> Result<Option<AuthorityEvent<P, R>>, TransportError> {
        Ok(self.receive.recv().await)
    }
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
    async fn receive(&mut self) -> Result<Option<DiscoveryReply>, TransportError> {
        Ok(self.0.recv().await)
    }
}
impl<P: Send + 'static, R: Send + 'static> ClientTransport<P, R> for Local<P, R> {
    type Discovery = Discovery;
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
