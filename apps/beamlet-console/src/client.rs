//! 发现收集多份回复; 提交只发送一次, 并校验回复关联.
use anyhow::{Context, Result, ensure};
use beamlet_protocol::{
    AuthorityInfo, Command, Discovery, DiscoveryReply, Outcome, Pool, Reply, RoleInstance, decode,
    encode, subject,
};
use futures::StreamExt;
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};
use tokio::time::{Instant, timeout, timeout_at};
use uuid::Uuid;

pub(crate) async fn discover(
    client: &async_nats::Client,
    pools: &BTreeSet<Pool>,
    request_id: Uuid,
) -> Result<Vec<AuthorityInfo>> {
    let inbox = client.new_inbox();
    let mut replies = client.subscribe(inbox.clone()).await?;
    client.flush().await?;
    let request = Discovery { request_id };
    let bytes = encode(&request)?;
    for pool in pools {
        client
            .publish_with_reply(
                subject::discovery(pool),
                inbox.clone(),
                bytes.clone().into(),
            )
            .await?;
    }
    let deadline = Instant::now() + Duration::from_secs(1);
    let mut authorities: BTreeMap<RoleInstance, AuthorityInfo> = BTreeMap::new();
    while let Ok(Some(message)) = timeout_at(deadline, replies.next()).await {
        let Ok(reply) = decode::<DiscoveryReply>(&message.payload) else {
            continue;
        };
        if reply.request_id != request.request_id
            || reply.authority.instance.role_id.is_nil()
            || reply.authority.instance.session_id.is_nil()
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
}

pub(crate) async fn request(
    client: &async_nats::Client,
    command: &Command,
    request_timeout: Duration,
) -> Result<Outcome> {
    let target = subject::authority(command.pool(), command.authority());
    let request_id = command.request_id();
    tracing::info!(event = "request.send_started", %request_id, "发送请求");
    let message = timeout(
        request_timeout,
        client.send_request(
            target,
            async_nats::Request::new()
                .payload(encode(command)?.into())
                .timeout(Some(request_timeout)),
        ),
    )
    .await
    .context("等待 Authority 超时; 请求可能已执行, 不会自动重发")?
    .context("请求未取得回复; 不会自动重发")?;
    let reply: Reply = decode(&message.payload).context("无法解析 Authority 回复")?;
    ensure!(
        reply.request_id == request_id && reply.authority == command.authority(),
        "回复的请求或 Authority 会话不匹配"
    );
    Ok(reply.outcome)
}
