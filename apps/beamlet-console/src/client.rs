//! 发现收集多份回复; 提交只发送一次, 并校验回复关联.
use anyhow::{Context, Result, ensure};
use beamlet_api::{
    AuthorityInfo, ClientTransport, Command, DiscoveryInbox, Outcome, Pool, RoleInstance,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};
use tokio::time::{Instant, timeout, timeout_at};
use uuid::Uuid;

pub(crate) async fn discover(
    client: &impl ClientTransport,
    pools: &BTreeSet<Pool>,
    request_id: Uuid,
    discovery_window: Duration,
) -> Result<Vec<AuthorityInfo>> {
    let mut replies = client.discover(pools, request_id).await?;
    let deadline = Instant::now() + discovery_window;
    let mut authorities: BTreeMap<RoleInstance, AuthorityInfo> = BTreeMap::new();
    while let Ok(response) = timeout_at(deadline, replies.receive()).await {
        let Some(reply) = response? else {
            break;
        };
        if reply.request_id != request_id
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
    client: &impl ClientTransport,
    command: &Command,
    request_timeout: Duration,
) -> Result<Outcome> {
    let request_id = command.request_id();
    tracing::info!(event = "request.send_started", %request_id, "发送请求");
    let reply = timeout(
        request_timeout,
        client.request(command.clone(), request_timeout),
    )
    .await
    .context("等待 Authority 超时; 请求可能已执行, 不会自动重发")?
    .context("请求未取得回复; 不会自动重发")?;
    ensure!(
        reply.request_id == request_id && reply.authority == command.authority(),
        "回复的请求或 Authority 会话不匹配"
    );
    Ok(reply.outcome)
}
