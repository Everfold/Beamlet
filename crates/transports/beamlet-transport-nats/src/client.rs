use crate::{Nats, subject};
use beamlet_api::{
    ClientTransport, Command, DiscoveryInbox, DiscoveryReply, Pool, Reply, TransportError, Uuid,
};
use beamlet_protocol::{decode, encode};
use futures::StreamExt;
use std::{collections::BTreeSet, time::Duration};

/// 一次发现查询的独立订阅, Drop 释放入口.
#[derive(Debug)]
pub struct Discovery(
    /// 本次发现的独立 NATS inbox 订阅.
    async_nats::Subscriber,
);
impl DiscoveryInbox for Discovery {
    /// 从 NATS 发现订阅中读取下一份可解码的回复.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Discovery` 的可变引用.
    ///
    /// # 返回值
    ///
    /// 有效回复为 `Ok(Some(reply))`, 订阅结束为 `Ok(None)`; 无效报文被跳过.
    async fn receive(&mut self) -> Result<Option<DiscoveryReply>, TransportError> {
        while let Some(message) = self.0.next().await {
            if let Ok(reply) = decode::<beamlet_protocol::DiscoveryReply>(&message.payload) {
                return Ok(Some(DiscoveryReply {
                    request_id: reply.request_id,
                    authority: reply.authority,
                }));
            }
        }
        Ok(None)
    }
}
impl ClientTransport for Nats {
    /// NATS 的独立发现回复订阅, 丢弃时释放本次查询入口.
    type Discovery = Discovery;
    /// 创建 NATS 回复订阅并向目标 pools 广播发现请求.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Nats` 的共享引用.
    /// - `pools`: 本次发现或登记关注的 pool 集合, 按精确名称匹配.
    /// - `request_id`: 请求与回复关联 UUID; 不作为 Operation 身份或幂等凭据.
    ///
    /// # 返回值
    ///
    /// 成功返回 Discovery; 订阅, 刷新, 编码或发送失败返回 TransportError.
    async fn discover(
        &self,
        pools: &BTreeSet<Pool>,
        request_id: Uuid,
    ) -> Result<Discovery, TransportError> {
        let inbox = self.client.new_inbox();
        let subscription = self
            .client
            .subscribe(inbox.clone())
            .await
            .map_err(TransportError::new)?;
        self.flush().await?;
        let bytes =
            encode(&beamlet_protocol::Discovery { request_id }).map_err(TransportError::new)?;
        for pool in pools {
            self.client
                .publish_with_reply(
                    subject::discovery(pool),
                    inbox.clone(),
                    bytes.clone().into(),
                )
                .await
                .map_err(TransportError::new)?;
        }
        Ok(Discovery(subscription))
    }
    /// 向指定 Authority 的 NATS 主题发送 v3 命令并等待回复.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Nats` 的共享引用.
    /// - `command`: 本次 Authority 命令, 携带请求 UUID, 目标会话及查询/提交数据.
    /// - `deadline`: 等待本次回复的最长时长, 非绝对时间; 超时不取消远端执行.
    ///
    /// # 返回值
    ///
    /// 成功返回解码后的 Reply; 期限, 编码, 通信或解码失败返回 TransportError.
    async fn request(&self, command: Command, deadline: Duration) -> Result<Reply, TransportError> {
        if deadline.is_zero() || std::time::Instant::now().checked_add(deadline).is_none() {
            return Err(TransportError::new("invalid request deadline"));
        }
        let target = subject::authority(command.pool(), command.authority());
        let bytes =
            encode(&beamlet_protocol::Command::from(command)).map_err(TransportError::new)?;
        let request = async_nats::Request::new()
            .payload(bytes.into())
            .timeout(Some(deadline));
        let response = tokio::time::timeout(deadline, self.client.send_request(target, request))
            .await
            .map_err(TransportError::new)?
            .map_err(TransportError::new)?;
        let reply: beamlet_protocol::Reply =
            decode(&response.payload).map_err(TransportError::new)?;
        Ok(reply.into())
    }
}
