//! 真实 Console 与 NATS; Authority 回复由测试控制, 证明首个结果返回前已有多个提交.
#![cfg(unix)]

use anyhow::{Context, Result, ensure};
use beamlet_api::{
    AuthorityInfo, Pool, Presence, PresenceState, RoleInstance, Selection, WorkerView,
};
use beamlet_protocol::{Command, Discovery, DiscoveryReply, Outcome, Reply, decode, encode};
use beamlet_transport_nats::subject;
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
    /// 结束测试服务子进程并回收其退出状态。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `ChildGuard` 的可变引用。
    ///
    /// # 返回值
    ///
    /// `()`；尽力终止和等待子进程，不把清理错误向外传播。
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
/// 在测试期限内反复连接本地 NATS 服务。
///
/// # 参数
///
/// - `port`：测试 NATS 服务监听的本地 TCP 端口。
///
/// # 返回值
///
/// 成功返回 async_nats 客户端；服务未及时启动时返回错误。
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
/// 等待测试日志中出现指定文本。
///
/// # 参数
///
/// - `path`：测试进程输出的日志文件路径。
/// - `expected`：日志中需要出现的文本子串。
///
/// # 返回值
///
/// 成功返回当前完整日志；文件读取失败或期限耗尽时返回错误。
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
/// 在测试期限内读取 NATS 订阅的下一条消息。
///
/// # 参数
///
/// - `input`：等待消息的测试 NATS 订阅。
///
/// # 返回值
///
/// 成功返回消息；订阅结束或超时返回错误。
async fn next(input: &mut async_nats::Subscriber) -> Result<async_nats::Message> {
    timeout(Duration::from_secs(5), input.next())
        .await?
        .context("subscription closed")
}
/// 按测试请求的回复地址发送关联正确的 v3 结果。
///
/// # 参数
///
/// - `client`：已连接测试 NATS 服务的原生客户端。
/// - `message`：接收到的 Authority 请求消息，其 reply 是回应地址。
/// - `outcome`：准备回应给本次请求的业务或查询结果。
///
/// # 返回值
///
/// 成功返回 `Ok(())`；解码、缺少回复地址、编码或发送失败返回错误。
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
/// 用给定数值构造可重复的测试角色身份。
///
/// # 参数
///
/// - `id`：确定性角色 UUID 的数值；会话身份使用该值加 100。
///
/// # 返回值
///
/// 角色 UUID 使用 id、会话 UUID 使用 id 加 100 的 RoleInstance。
fn instance(id: u128) -> RoleInstance {
    RoleInstance {
        role_id: Uuid::from_u128(id),
        session_id: Uuid::from_u128(id + 100),
    }
}
/// 构造供真实 Console 测试展示的一份 Worker 列表回复。
///
/// # 返回值
///
/// 包含预设 Ready Worker 和文本倒序能力的 Outcome::Workers。
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
/// 构造带预设关联身份的已完成提交结果。
///
/// # 参数
///
/// - `text`：作为成功业务结果展示的测试文本。
///
/// # 返回值
///
/// 携带输入文本作为业务结果的 Outcome::Completed。
fn completed(text: &str) -> Outcome {
    Outcome::Completed {
        operation_id: format!("operation:{text}"),
        attempt_id: format!("attempt:{text}"),
        report_id: format!("report:{text}"),
        worker: instance(2),
        result: text.into(),
    }
}

/// 验证真实 Console 同时提交多个请求，并按关联身份显示乱序结果和退出排空。
///
/// # 返回值
///
/// 所有断言通过时返回 `Ok(())`；I/O 或协议验证失败返回错误，断言失败会使测试失败。
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
    for id in [fast_id, slow_id, unanswered_id] {
        ensure!(
            output
                .lines()
                .any(|line| line.contains("event=\"request.send_started\"")
                    && line.contains(&id.to_string())),
            "Client send event missing for {id}: {output}"
        );
    }
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
