//! Node 负责装配、监督和统一退出; 角色保留独立的业务状态与身份.
use crate::{
    infrastructure::Nats,
    role::RunningRole,
    settings::{Infrastructure, Settings},
    shutdown, terminal,
};
use anyhow::{Context, Result};
use std::{collections::BTreeMap, future::Future, time::Duration};
use tokio::{
    sync::watch,
    task::JoinSet,
    time::{Instant, sleep_until, timeout},
};
use tracing::{Instrument, info, warn};

pub(crate) async fn run(
    settings: Settings,
    signal: impl Future<Output = std::io::Result<()>>,
) -> Result<()> {
    settings.validate()?;
    info!(
        event = "node.starting",
        roles = settings.roles.len(),
        infrastructures = settings.infrastructures.len(),
        "Node 启动"
    );
    let roles: Vec<_> = settings.roles.into_iter().map(RunningRole::new).collect();
    for role in &roles {
        info!(event = "role.prepared", role_id = %role.instance.role_id, role_session_id = %role.instance.session_id,
            kind = role.settings.kind.name(), label = role.settings.label.as_deref(), infrastructure = %role.settings.infrastructure, "已分配角色会话");
    }
    tokio::pin!(signal);
    let infrastructure = async {
        let mut connected = BTreeMap::new();
        for (name, config) in settings.infrastructures {
            let span = tracing::info_span!("infrastructure", name = %name);
            let connection = match config {
                Infrastructure::Nats {
                    url,
                    connect_timeout_secs,
                    subscription_capacity,
                } => {
                    let deadline = Duration::from_secs(connect_timeout_secs);
                    timeout(
                        deadline,
                        Nats::connect(&url, deadline, subscription_capacity).instrument(span),
                    )
                    .await
                    .context("连接基础设施超时")??
                }
            };
            info!(event = "infrastructure.ready", %name, "基础设施已就绪");
            connected.insert(name, connection);
        }
        Ok::<_, anyhow::Error>(connected)
    };
    let infrastructures = tokio::select! {
        result = &mut signal => { result?; return Ok(()); }
        result = infrastructure => result?,
    };
    let (stop, shutdown) = shutdown::channel();
    let mut tasks = JoinSet::new();
    let mut targets = Vec::new();
    for role in roles {
        let infrastructure = infrastructures
            .get(&role.settings.infrastructure)
            .context("已校验的基础设施缺失")?
            .clone();
        let (target, local) = role.local_input();
        targets.extend(target);
        tasks.spawn(
            role.run(infrastructure, local, shutdown.clone())
                .in_current_span(),
        );
    }
    let result = supervise(
        tasks,
        stop,
        terminal::run(targets, shutdown),
        signal,
        Duration::from_secs(settings.shutdown_timeout_secs),
    )
    .await;
    for (name, infrastructure) in &infrastructures {
        match timeout(Duration::from_secs(1), infrastructure.client.flush()).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                warn!(event = "infrastructure.flush_failed", %name, %error, "退出时刷新失败")
            }
            Err(error) => {
                warn!(event = "infrastructure.flush_failed", %name, %error, "退出时刷新超时")
            }
        }
    }
    info!(event = "node.stopped", "所有角色已退出");
    result
}

async fn supervise(
    mut tasks: JoinSet<Result<()>>,
    stop: watch::Sender<bool>,
    local: impl Future<Output = Result<()>>,
    signal: impl Future<Output = std::io::Result<()>>,
    shutdown_timeout: Duration,
) -> Result<()> {
    tokio::pin!(signal, local);
    let mut stopping = false;
    let mut local_done = false;
    let mut failure = None;
    let mut deadline = Instant::now();
    loop {
        if stopping && tasks.is_empty() {
            return failure.map_or(Ok(()), Err);
        }
        let error = tokio::select! {
            result = &mut signal, if !stopping => { result.err().map(anyhow::Error::from) }
            result = &mut local, if !local_done && !stopping => {
                local_done = true;
                match result { Ok(()) => continue, Err(error) => Some(error) }
            }
            result = tasks.join_next(), if !tasks.is_empty() => {
                match result.context("缺少角色任务")? {
                    Ok(Ok(())) if stopping => continue,
                    Ok(Ok(())) => Some(anyhow::anyhow!("角色意外退出")),
                    Ok(Err(error)) => Some(error),
                    Err(error) => Some(error.into()),
                }
            }
            _ = sleep_until(deadline), if stopping => {
                tasks.abort_all();
                while tasks.join_next().await.is_some() {}
                return Err(failure.unwrap_or_else(|| anyhow::anyhow!("角色排空超时, Node 已停止等待; 未完成操作没有自动重试")));
            }
        };
        if failure.is_none() {
            failure = error;
        }
        if !stopping {
            stopping = true;
            deadline = Instant::now() + shutdown_timeout;
            stop.send_replace(true);
            info!(event = "node.stopping", "通知所有角色停止新接收并排空");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    #[tokio::test]
    async fn one_failed_role_stops_siblings_and_reports_the_original_error() {
        let (stop, shutdown) = shutdown::channel();
        let drained = Arc::new(AtomicUsize::new(0));
        let mut tasks = JoinSet::new();
        for _ in 0..3 {
            let shutdown = shutdown.clone();
            let drained = drained.clone();
            tasks.spawn(async move {
                shutdown.wait().await?;
                drained.fetch_add(1, Ordering::SeqCst);
                Ok(())
            });
        }
        tasks.spawn(async { anyhow::bail!("broken role") });
        let result = supervise(
            tasks,
            stop,
            async { Ok(()) },
            std::future::pending(),
            Duration::from_secs(6),
        )
        .await;
        assert!(result.unwrap_err().to_string().contains("broken role"));
        assert_eq!(drained.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn eof_does_not_stop_roles_and_shutdown_drains_all_of_them() {
        let (stop, shutdown) = shutdown::channel();
        let (signal, receive) = tokio::sync::oneshot::channel();
        let drained = Arc::new(AtomicUsize::new(0));
        let mut tasks = JoinSet::new();
        for _ in 0..2 {
            let shutdown = shutdown.clone();
            let drained = drained.clone();
            tasks.spawn(async move {
                shutdown.wait().await?;
                drained.fetch_add(1, Ordering::SeqCst);
                Ok(())
            });
        }
        let running = tokio::spawn(supervise(
            tasks,
            stop,
            async { Ok(()) },
            async {
                receive.await.expect("test signal sender");
                Ok(())
            },
            Duration::from_secs(6),
        ));
        tokio::task::yield_now().await;
        assert!(!running.is_finished());
        signal.send(()).unwrap();
        running.await.unwrap().unwrap();
        assert_eq!(drained.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn infrastructure_only_node_waits_for_exit_and_propagates_signal_error() {
        let (stop, _shutdown) = shutdown::channel();
        let result = supervise(
            JoinSet::new(),
            stop,
            async { Ok(()) },
            async { Err(std::io::Error::other("signal unavailable")) },
            Duration::from_secs(6),
        )
        .await;
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("signal unavailable")
        );
    }

    #[tokio::test]
    async fn panicked_role_requests_shutdown_for_its_siblings() {
        let (stop, shutdown) = shutdown::channel();
        let (send, received) = tokio::sync::oneshot::channel();
        let mut tasks = JoinSet::new();
        tasks.spawn(async move {
            shutdown.wait().await?;
            send.send(()).expect("test receiver");
            Ok(())
        });
        tasks.spawn(async { panic!("simulated role panic") });
        let result = supervise(
            tasks,
            stop,
            async { Ok(()) },
            std::future::pending(),
            Duration::from_secs(6),
        )
        .await;
        assert!(result.is_err());
        received.await.unwrap();
    }
    #[tokio::test]
    async fn configured_drain_deadline_stops_waiting_for_an_unfinished_role() {
        let (stop, _) = shutdown::channel();
        let mut tasks = JoinSet::new();
        tasks.spawn(std::future::pending());
        let result = timeout(
            Duration::from_secs(1),
            supervise(
                tasks,
                stop,
                std::future::pending(),
                async { Ok(()) },
                Duration::from_millis(20),
            ),
        )
        .await
        .expect("configured shutdown deadline was ignored");
        assert!(result.unwrap_err().to_string().contains("角色排空超时"));
    }
}
