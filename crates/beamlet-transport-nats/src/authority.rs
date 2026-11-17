use crate::{Nats, subject};
use beamlet_api::{
    AuthorityEvent, AuthorityInbox, AuthorityInfo, ConnectionView, DiscoveryReply, Pool,
    ReplyError, Responder, TransportError,
};
use beamlet_protocol::{decode, encode};
use futures::{FutureExt, StreamExt, stream::SelectAll};
use std::collections::{BTreeMap, BTreeSet};

/// Authority NATS 订阅的消息种类, 用于选择对应解码与控制事件.
#[derive(Debug, Clone, Copy)]
enum Kind {
    /// Authority 发现请求, 解码后保留多回复入口.
    Discovery,
    /// Worker 完整状态公告, 按所在 pool 交给 Authority 登记.
    Presence,
    /// 发往唯一 Authority 会话的查询或提交命令.
    Request,
}

/// Authority 的原始控制事件入口, 与业务任务准入队列分离.
#[derive(Debug)]
pub struct AuthorityEndpoint {
    /// 本端点共享的 NATS 连接句柄.
    nats: Nats,
    /// 本 Authority 绑定的非空 pool 集合.
    pools: BTreeSet<Pool>,
    /// 发现, 公告与请求订阅的合并输入流.
    subscriptions: SelectAll<async_nats::Subscriber>,
    /// 已订阅主题到 pool 和控制消息种类的映射.
    routes: BTreeMap<String, (Pool, Kind)>,
    /// 最近处理的连接世代, 用于使重连前缓冲失效.
    epoch: u64,
}
impl AuthorityEndpoint {
    /// 订阅 Authority 各个 pool 的控制主题并刷新订阅.
    ///
    /// # 参数
    ///
    /// - `nats`: 已连接的 NATS 客户端及连接状态句柄.
    /// - `info`: Authority 的角色/会话身份, 标签及其加入的 pools.
    ///
    /// # 返回值
    ///
    /// 成功返回 AuthorityEndpoint; 绑定非法, 订阅或刷新失败返回 TransportError.
    pub(crate) async fn bind(mut nats: Nats, info: &AuthorityInfo) -> Result<Self, TransportError> {
        if info.instance.role_id.is_nil()
            || info.instance.session_id.is_nil()
            || info.pools.is_empty()
        {
            return Err(TransportError::new("invalid authority binding"));
        }
        let mut subscriptions = SelectAll::new();
        let mut routes = BTreeMap::new();
        let queue = format!(
            "beamlet-authority-{}-{}",
            info.instance.role_id, info.instance.session_id
        );
        for pool in &info.pools {
            for (subject, kind) in [
                (subject::discovery(pool), Kind::Discovery),
                (subject::presence(pool), Kind::Presence),
                (subject::authority(pool, info.instance), Kind::Request),
            ] {
                // 同一会话的多个连接只分发一次请求; 公告仍向所有连接广播.
                let subscription = match kind {
                    Kind::Request => {
                        nats.client
                            .queue_subscribe(subject.clone(), queue.clone())
                            .await
                    }
                    Kind::Discovery | Kind::Presence => {
                        nats.client.subscribe(subject.clone()).await
                    }
                }
                .map_err(TransportError::new)?;
                subscriptions.push(subscription);
                routes.insert(subject, (pool.clone(), kind));
            }
        }
        nats.flush().await?;
        let epoch = nats.status.borrow_and_update().epoch;
        Ok(Self {
            nats,
            pools: info.pools.clone(),
            subscriptions,
            routes,
            epoch,
        })
    }
}
impl AuthorityInbox for AuthorityEndpoint {
    /// 取得能够持续读取当前连接状态的视图.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `AuthorityEndpoint` 的共享引用.
    ///
    /// # 返回值
    ///
    /// 连接视图句柄; 后续查询反映连接当前世代和状态.
    fn connection(&self) -> ConnectionView {
        self.nats.connection()
    }
    /// 合并 NATS 控制订阅与连接变化, 丢弃重连前的旧缓冲.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `AuthorityEndpoint` 的可变引用.
    ///
    /// # 返回值
    ///
    /// 有有效事件返回 `Ok(Some(event))`; 订阅结束返回 `Ok(None)`; 连接观察失败返回错误.
    async fn receive(&mut self) -> Result<Option<AuthorityEvent>, TransportError> {
        loop {
            let message = tokio::select! {
                biased;
                changed = self.nats.status.changed() => {
                    changed.map_err(TransportError::new)?;
                    let link = *self.nats.status.borrow_and_update();
                    self.epoch = link.epoch;
                    // 重连前的缓冲不能为新世代续租. 事件返回前不等待任何发送.
                    while self.subscriptions.next().now_or_never().flatten().is_some() {}
                    return Ok(Some(AuthorityEvent::Connection(link)));
                }
                message = self.subscriptions.next() => match message { Some(message) => message, None => return Ok(None) },
            };
            if !self.nats.status.borrow().connected {
                continue;
            }
            let Some((pool, kind)) = self.routes.get(message.subject.as_str()) else {
                continue;
            };
            let client = self.nats.client.clone();
            match kind {
                Kind::Presence => {
                    let presence: beamlet_api::Presence = match decode(&message.payload) {
                        Ok(presence) => presence,
                        Err(error) => {
                            tracing::warn!(event = "heartbeat.rejected", %pool, %error, "忽略无效心跳");
                            continue;
                        }
                    };
                    if &presence.pool == pool {
                        return Ok(Some(AuthorityEvent::Presence {
                            presence,
                            epoch: self.epoch,
                        }));
                    }
                }
                Kind::Discovery => {
                    let (Some(subject), Ok(request)) = (
                        message.reply,
                        decode::<beamlet_protocol::Discovery>(&message.payload),
                    ) else {
                        continue;
                    };
                    return Ok(Some(AuthorityEvent::Discovery {
                        pool: pool.clone(),
                        request_id: request.request_id,
                        reply: Responder::new(move |reply: DiscoveryReply| async move {
                            let wire = beamlet_protocol::DiscoveryReply {
                                request_id: reply.request_id,
                                authority: reply.authority,
                            };
                            client
                                .publish(subject, encode(&wire).map_err(ReplyError::new)?.into())
                                .await
                                .map_err(ReplyError::new)
                        }),
                    }));
                }
                Kind::Request => {
                    let Some(subject) = message.reply else {
                        continue;
                    };
                    let command: beamlet_protocol::Command = match decode(&message.payload) {
                        Ok(command) => command,
                        Err(error) => {
                            tracing::warn!(event = "request.rejected", %error, "忽略无效请求");
                            continue;
                        }
                    };
                    return Ok(Some(AuthorityEvent::Request {
                        pool: pool.clone(),
                        command: command.into(),
                        reply: Responder::new(move |reply: beamlet_api::Reply| async move {
                            let request_id = reply.request_id;
                            client
                                .publish(
                                    subject,
                                    encode(&beamlet_protocol::Reply::from(reply))
                                        .map_err(ReplyError::new)?
                                        .into(),
                                )
                                .await
                                .map_err(ReplyError::new)?;
                            tracing::info!(event = "response.sent", %request_id, "已回复远端请求");
                            Ok(())
                        }),
                    }));
                }
            }
        }
    }
    /// 请求所绑定 pools 中的 Worker 重新发布完整快照.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `AuthorityEndpoint` 的共享引用.
    ///
    /// # 返回值
    ///
    /// 发送/通知完成返回 `Ok(())`; 传输失败返回错误, 不保证 Worker 已回复.
    async fn probe(&self) -> Result<(), TransportError> {
        self.nats.flush().await?;
        for pool in &self.pools {
            self.nats
                .client
                .publish(subject::probe(pool), Vec::new().into())
                .await
                .map_err(TransportError::new)?;
        }
        Ok(())
    }
}
