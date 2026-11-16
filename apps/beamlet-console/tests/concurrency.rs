//! 真实 Console 与 NATS; Authority 回复由测试控制, 证明首个结果返回前已有多个提交.
#![cfg(unix)]

use anyhow::{Context, Result, ensure};
use beamlet_protocol::{
    AuthorityInfo, Command, Discovery, DiscoveryReply, Outcome, Pool, Presence, PresenceState,
    Reply, RoleInstance, Selection, WorkerView, decode, encode, subject,
};
use futures::StreamExt;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    net::TcpListener,
    path::Path,
    process::{Child, Command as Process, Stdio},
    time::Duration,
};
use tokio::time::{Instant, sleep, timeout};
use uuid::Uuid;

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
async fn connect(port: u16) -> Result<async_nats::Client> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(client) = async_nats::connect(format!("127.0.0.1:{port}")).await {
            return Ok(client);
        }
        ensure!(Instant::now() < deadline, "NATS did not start");
        sleep(Duration::from_millis(40)).await;
    }
}
async fn output_contains(path: &Path, expected: &str) -> Result<String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let output = fs::read_to_string(path)?;
        if output.contains(expected) {
            return Ok(output);
        }
        ensure!(Instant::now() < deadline, "missing {expected:?}:\n{output}");
        sleep(Duration::from_millis(20)).await;
    }
}
async fn next(input: &mut async_nats::Subscriber) -> Result<async_nats::Message> {
    timeout(Duration::from_secs(5), input.next())
        .await?
        .context("subscription closed")
}
async fn reply(
    client: &async_nats::Client,
    message: &async_nats::Message,
    outcome: Outcome,
) -> Result<()> {
    let command: Command = decode(&message.payload)?;
    client
        .publish(
            message.reply.clone().context("missing reply subject")?,
            encode(&Reply {
                request_id: command.request_id(),
                authority: command.authority(),
                outcome,
            })?
            .into(),
        )
        .await?;
    Ok(())
}
fn instance(id: u128) -> RoleInstance {
    RoleInstance {
        role_id: Uuid::from_u128(id),
        session_id: Uuid::from_u128(id + 100),
    }
}
fn workers() -> Outcome {
    Outcome::Workers {
        workers: vec![WorkerView {
            presence: Presence {
                pool: Pool::default(),
                instance: instance(2),
                label: Some("worker".into()),
                capabilities: BTreeSet::from(["text.reverse/v1".into()]),
                sequence: 1,
                state: PresenceState::Ready,
            },
            last_seen_ms: 0,
            stale: false,
        }],
    }
}
fn completed(text: &str) -> Outcome {
    Outcome::Completed {
        operation_id: format!("operation:{text}"),
        attempt_id: format!("attempt:{text}"),
        report_id: format!("report:{text}"),
        worker: instance(2),
        result: text.into(),
    }
}

#[tokio::test]
#[ignore = "requires nats-server; explicit Console integration test"]
async fn console_submits_concurrently_and_drains_correlated_results() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let port = TcpListener::bind("127.0.0.1:0")?.local_addr()?.port();
    let _broker = ChildGuard(
        Process::new("nats-server")
            .args(["-a", "127.0.0.1", "-p", &port.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("install nats-server on PATH")?,
    );
    let client = connect(port).await?;
    let mut discovery = client
        .subscribe(subject::discovery(&Pool::default()))
        .await?;
    let mut requests = client
        .subscribe(subject::authority(&Pool::default(), instance(1)))
        .await?;
    client.flush().await?;
    let binary = directory.path().join("beamlet-console");
    fs::copy(env!("CARGO_BIN_EXE_beamlet-console"), &binary)?;
    let config = format!(
        r#"(nats_url: "nats://127.0.0.1:{port}", pools: ["default"], color: never, request_timeout_secs: 2, max_in_flight_requests: 4, discovery_window_ms: 200, connect_timeout_secs: 12, subscription_capacity: 32)"#
    );
    let config_path = directory.path().join("console.ron");
    fs::write(&config_path, &config)?;
    let log_path = directory.path().join("console.log");
    let output = fs::File::create(&log_path)?;
    let mut console = ChildGuard(
        Process::new(binary)
            .stdin(Stdio::piped())
            .stdout(output.try_clone()?)
            .stderr(output)
            .spawn()?,
    );
    let mut input = console.0.stdin.take().context("Console stdin missing")?;

    let found = next(&mut discovery).await?;
    let query: Discovery = decode(&found.payload)?;
    client
        .publish(
            found.reply.context("discovery reply missing")?,
            encode(&DiscoveryReply {
                request_id: query.request_id,
                authority: AuthorityInfo {
                    instance: instance(1),
                    label: Some("authority".into()),
                    pools: BTreeSet::from([Pool::default()]),
                },
            })?
            .into(),
        )
        .await?;
    output_contains(&log_path, "/authority <编号> →").await?;
    writeln!(input, "/authority 1\n/pool default")?;
    let query = next(&mut requests).await?;
    ensure!(
        matches!(decode::<Command>(&query.payload)?, Command::Workers { .. }),
        "expected worker query"
    );
    reply(&client, &query, workers()).await?;
    output_contains(&log_path, "  1. text.reverse/v1").await?;
    writeln!(
        input,
        "/capability 1\n/worker 1\nslow\nfast\n/workers\n/help"
    )?;

    // 所有提交和控制查询必须先到达, 此时还没有任何提交回复.
    let mut submissions = BTreeMap::new();
    let mut control = None;
    let mut ids = BTreeSet::new();
    for _ in 0..3 {
        let message = next(&mut requests).await?;
        let command: Command = decode(&message.payload)?;
        ensure!(ids.insert(command.request_id()), "duplicate request id");
        ensure!(command.authority() == instance(1), "authority changed");
        match command {
            Command::Submit { submission, .. } => {
                ensure!(
                    submission.worker == Selection::Exact(instance(2)),
                    "worker changed"
                );
                ensure!(
                    submission.task.capability == "text.reverse/v1",
                    "capability changed"
                );
                submissions.insert(submission.task.text, message);
            }
            Command::Workers { .. } => control = Some(message),
        }
    }
    ensure!(
        submissions.len() == 2 && control.is_some(),
        "requests did not overlap"
    );
    writeln!(input, "timeout")?;
    let unanswered = next(&mut requests).await?;
    let unanswered_id = decode::<Command>(&unanswered.payload)?.request_id();
    writeln!(input, "/quit")?;
    output_contains(&log_path, "event=\"console.draining\"").await?;
    ensure!(
        console.0.try_wait()?.is_none(),
        "Console quit before draining"
    );
    let fast = submissions.remove("fast").context("fast request missing")?;
    let fast_id = decode::<Command>(&fast.payload)?.request_id();
    reply(&client, &fast, completed("fast-result")).await?;
    output_contains(&log_path, &format!("[{fast_id}] fast-result")).await?;
    ensure!(
        console.0.try_wait()?.is_none(),
        "Console quit while slow request pending"
    );
    reply(
        &client,
        &control.context("control request missing")?,
        workers(),
    )
    .await?;
    let slow = submissions.remove("slow").context("slow request missing")?;
    let slow_id = decode::<Command>(&slow.payload)?.request_id();
    reply(&client, &slow, completed("slow-result")).await?;
    let output = output_contains(&log_path, "event=\"console.stopped\"").await?;
    ensure!(
        output
            .lines()
            .any(|line| line.contains(&unanswered_id.to_string())
                && line.contains("event=\"request.failed\"")),
        "configured 2-second request timeout was ignored: {output}"
    );
    ensure!(
        output.contains(&format!("[{slow_id}] slow-result")),
        "slow result correlation: {output}"
    );
    ensure!(
        output.find("fast-result") < output.find("slow-result"),
        "results not shown as completed"
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = console.0.try_wait()? {
            ensure!(status.success(), "Console failed:\n{output}");
            break;
        }
        ensure!(Instant::now() < deadline, "Console did not exit");
        sleep(Duration::from_millis(20)).await;
    }
    ensure!(
        fs::read_to_string(config_path)? == config,
        "config was rewritten"
    );
    Ok(())
}
