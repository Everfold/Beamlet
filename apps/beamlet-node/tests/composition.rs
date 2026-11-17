//! 显式验收: cargo test -p beamlet-node --test composition -- --ignored --nocapture
//! 要求 Unix 和 PATH 中的 nats-server; 普通 cargo test 不启动外部服务.
#![cfg(unix)]

use anyhow::{Context, Result, ensure};
use beamlet_api::{
    AuthorityInfo, Pool, Presence, PresenceState, RoleInstance, Selection, WorkerView,
};
use beamlet_protocol::{
    Command, Discovery, DiscoveryReply, Outcome, Submission, Task, decode, encode,
};
use beamlet_transport_nats::subject;
use futures::StreamExt;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    net::TcpListener,
    path::PathBuf,
    process::{Child, Command as Process, Stdio},
    time::Duration,
};
use tokio::time::{Instant, sleep, timeout_at};
use uuid::Uuid;

struct ProcessRecord {
    child: Child,
    log: PathBuf,
}
struct Cluster {
    root: tempfile::TempDir,
    processes: Vec<ProcessRecord>,
}
impl Cluster {
    fn new() -> Result<Self> {
        let root = tempfile::tempdir()?;
        fs::copy(
            env!("CARGO_BIN_EXE_beamlet-node"),
            root.path().join("beamlet-node"),
        )?;
        Ok(Self {
            root,
            processes: Vec::new(),
        })
    }
    fn spawn(&mut self, mut command: Process, name: &str) -> Result<usize> {
        let log = self.root.path().join(format!("{name}.log"));
        let output = fs::File::create(&log)?;
        let child = command
            .current_dir(self.root.path())
            .stdin(Stdio::piped())
            .stdout(output.try_clone()?)
            .stderr(output)
            .spawn()?;
        self.processes.push(ProcessRecord { child, log });
        Ok(self.processes.len() - 1)
    }
    fn broker(&mut self, port: u16, name: &str) -> Result<usize> {
        let mut command = Process::new("nats-server");
        command.args(["-a", "127.0.0.1", "-p", &port.to_string()]);
        self.spawn(command, name)
            .context("nats-server must be installed on PATH")
    }
    async fn node(
        &mut self,
        name: &str,
        config: &str,
        roles: usize,
    ) -> Result<(usize, BTreeMap<Uuid, RoleInstance>)> {
        fs::write(self.root.path().join(format!("{name}.ron")), config)?;
        let mut command = Process::new(self.root.path().join("beamlet-node"));
        command.args(["--config", &format!("{name}.ron")]);
        let process = self.spawn(command, name)?;
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            let record = &mut self.processes[process];
            let text = fs::read_to_string(&record.log)?;
            ensure!(record.child.try_wait()?.is_none(), "node stopped: {text}");
            if text.matches("event=\"role.ready\"").count() == roles {
                let instances = text
                    .lines()
                    .filter(|line| line.contains("event=\"role.prepared\""))
                    .map(|line| {
                        let role_id = uuid_field(line, "role_id");
                        (
                            role_id,
                            RoleInstance {
                                role_id,
                                session_id: uuid_field(line, "role_session_id"),
                            },
                        )
                    })
                    .collect();
                return Ok((process, instances));
            }
            ensure!(Instant::now() < deadline, "roles did not start: {text}");
            sleep(Duration::from_millis(40)).await;
        }
    }
    fn kill(&mut self, process: usize) -> Result<()> {
        self.processes[process].child.kill()?;
        self.processes[process].child.wait()?;
        Ok(())
    }
    async fn stop(&mut self, process: usize) -> Result<()> {
        let pid = self.processes[process].child.id();
        ensure!(
            Process::new("kill")
                .args(["-INT", &pid.to_string()])
                .status()?
                .success(),
            "could not interrupt child"
        );
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = self.processes[process].child.try_wait()? {
                ensure!(
                    status.success(),
                    "node shutdown failed: {}",
                    fs::read_to_string(&self.processes[process].log)?
                );
                return Ok(());
            }
            ensure!(Instant::now() < deadline, "node did not drain all roles");
            sleep(Duration::from_millis(40)).await;
        }
    }
}
impl Drop for Cluster {
    fn drop(&mut self) {
        for record in &mut self.processes {
            let _ = record.child.kill();
            let _ = record.child.wait();
            if std::thread::panicking() {
                eprintln!(
                    "{}:\n{}",
                    record.log.display(),
                    fs::read_to_string(&record.log).unwrap_or_default()
                );
            }
        }
    }
}
fn uuid_field(line: &str, name: &str) -> Uuid {
    line.split_whitespace()
        .find_map(|part| part.strip_prefix(&format!("{name}=")))
        .expect("identity log field")
        .parse()
        .expect("UUID field")
}
fn port() -> Result<u16> {
    Ok(TcpListener::bind("127.0.0.1:0")?.local_addr()?.port())
}
async fn connect(port: u16) -> Result<async_nats::Client> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(client) = async_nats::ConnectOptions::new()
            .request_timeout(Some(Duration::from_secs(5)))
            .connect(format!("127.0.0.1:{port}"))
            .await
        {
            return Ok(client);
        }
        ensure!(Instant::now() < deadline, "NATS did not start");
        sleep(Duration::from_millis(40)).await;
    }
}
fn role(id: Uuid, infrastructure: &str, kind: &str) -> String {
    format!(
        r#"(role_id: "{id}", label: Some("same label"), infrastructure: "{infrastructure}", kind: {kind})"#
    )
}
fn authority(pools: &str, timeout: u64, concurrency: usize) -> String {
    format!(
        "authority((pools: [{pools}], worker_timeout_secs: {timeout}, report_timeout_secs: 4, input_capacity: 7, max_concurrent_replies: 3, max_concurrent_tasks: {concurrency}))"
    )
}
fn worker(capability: &str, interval: u64, concurrency: usize) -> String {
    format!(
        r#"worker((pool: "blue", capabilities: ["{capability}"], heartbeat_interval_secs: {interval}, max_concurrent_tasks: {concurrency}))"#
    )
}
fn config(node_id: Uuid, infrastructure: &str, roles: &[String]) -> String {
    format!(
        r#"(node_id: "{node_id}", label: Some("composed node"), color: never, shutdown_timeout_secs: 8, infrastructures: {{{infrastructure}}}, roles: [{}])"#,
        roles.join(",")
    )
}
async fn discover(client: &async_nats::Client, pool: &Pool) -> Result<Vec<AuthorityInfo>> {
    let inbox = client.new_inbox();
    let mut replies = client.subscribe(inbox.clone()).await?;
    client.flush().await?;
    let request_id = Uuid::new_v4();
    client
        .publish_with_reply(
            subject::discovery(pool),
            inbox,
            encode(&Discovery { request_id })?.into(),
        )
        .await?;
    let deadline = Instant::now() + Duration::from_millis(250);
    let mut found = BTreeMap::new();
    while let Ok(Some(message)) = timeout_at(deadline, replies.next()).await {
        if let Ok(reply) = decode::<DiscoveryReply>(&message.payload) {
            ensure!(reply.request_id == request_id, "discovery correlation");
            found.insert(reply.authority.instance, reply.authority);
        }
    }
    Ok(found.into_values().collect())
}
async fn request(client: &async_nats::Client, command: Command) -> Result<Outcome> {
    let message = client
        .request(
            subject::authority(command.pool(), command.authority()),
            encode(&command)?.into(),
        )
        .await?;
    let reply: beamlet_protocol::Reply = decode(&message.payload)?;
    ensure!(
        reply.authority == command.authority() && reply.request_id == command.request_id(),
        "reply correlation"
    );
    Ok(reply.outcome)
}
async fn workers(
    client: &async_nats::Client,
    authority: RoleInstance,
    pool: &Pool,
) -> Result<Vec<WorkerView>> {
    let outcome = request(
        client,
        Command::Workers {
            request_id: Uuid::new_v4(),
            authority,
            pool: pool.clone(),
            capability: None,
        },
    )
    .await?;
    let Outcome::Workers { workers } = outcome else {
        anyhow::bail!("unexpected query reply: {outcome:?}");
    };
    Ok(workers)
}
async fn wait_for_workers(
    client: &async_nats::Client,
    authority: RoleInstance,
    pool: &Pool,
    expected: &[RoleInstance],
) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let view = workers(client, authority, pool).await?;
        if expected.iter().all(|instance| {
            view.iter().any(|item| {
                item.presence.instance == *instance
                    && !item.stale
                    && item.presence.state == PresenceState::Ready
            })
        }) {
            return Ok(());
        }
        ensure!(
            Instant::now() < deadline,
            "workers not registered: {view:?}"
        );
        sleep(Duration::from_millis(60)).await;
    }
}
async fn submit(
    client: &async_nats::Client,
    authority: RoleInstance,
    pool: &Pool,
    worker: RoleInstance,
    capability: &str,
    text: &str,
) -> Result<Outcome> {
    request(
        client,
        Command::Submit {
            request_id: Uuid::new_v4(),
            authority,
            submission: Submission {
                pool: pool.clone(),
                worker: Selection::Exact(worker),
                task: Task {
                    capability: capability.into(),
                    text: text.into(),
                },
            },
        },
    )
    .await
}
fn completed(outcome: Outcome, instance: RoleInstance, expected: &str) -> Result<String> {
    let Outcome::Completed {
        worker,
        result,
        operation_id,
        ..
    } = outcome
    else {
        anyhow::bail!("expected completion: {outcome:?}");
    };
    ensure!(
        worker == instance && result == expected,
        "unexpected execution result"
    );
    Ok(operation_id)
}

#[tokio::test]
#[ignore = "requires nats-server; explicit multi-process integration test"]
async fn multiple_roles_and_infrastructures_route_restart_and_drain_independently() -> Result<()> {
    let mut cluster = Cluster::new()?;
    let primary_port = port()?;
    let secondary_port = port()?;
    let primary_broker = cluster.broker(primary_port, "primary")?;
    let _secondary_broker = cluster.broker(secondary_port, "secondary")?;
    let primary = connect(primary_port).await?;
    let secondary = connect(secondary_port).await?;
    let pool: Pool = "blue".parse()?;
    let ids: Vec<_> = (0..6).map(|_| Uuid::new_v4()).collect();
    let node_id = Uuid::new_v4();
    let infrastructure = format!(
        r#""primary": nats(url: "nats://127.0.0.1:{primary_port}", connect_timeout_secs: 12, subscription_capacity: 32), "worker-connection": nats(url: "nats://127.0.0.1:{primary_port}", connect_timeout_secs: 12, subscription_capacity: 32), "secondary": nats(url: "nats://127.0.0.1:{secondary_port}", connect_timeout_secs: 12, subscription_capacity: 32)"#
    );
    let roles = vec![
        role(
            ids[0],
            "primary",
            &authority(r#""blue", "red", "blue""#, 3, 1),
        ),
        role(ids[1], "primary", &authority(r#""blue""#, 5, 4)),
        role(ids[2], "primary", &worker("text.uppercase/v1", 1, 4)),
        role(
            ids[3],
            "worker-connection",
            &worker("text.reverse/v1", 2, 1),
        ),
        role(ids[4], "secondary", &authority(r#""blue""#, 3, 0)),
        role(ids[5], "secondary", &worker("text.reverse/v1", 1, 0)),
    ];
    let source = config(node_id, &infrastructure, &roles);
    let (first_process, first) = cluster.node("composed", &source, 6).await?;
    ensure!(
        first
            .values()
            .map(|instance| instance.session_id)
            .collect::<BTreeSet<_>>()
            .len()
            == 6,
        "sessions are shared"
    );
    wait_for_workers(
        &primary,
        first[&ids[0]],
        &pool,
        &[first[&ids[2]], first[&ids[3]]],
    )
    .await?;
    wait_for_workers(
        &primary,
        first[&ids[1]],
        &pool,
        &[first[&ids[2]], first[&ids[3]]],
    )
    .await?;
    wait_for_workers(&secondary, first[&ids[4]], &pool, &[first[&ids[5]]]).await?;
    ensure!(
        discover(&primary, &pool).await?.len() == 2,
        "same Node authorities collapsed"
    );
    ensure!(
        discover(&secondary, &pool).await?.len() == 1,
        "infrastructure leaked"
    );
    ensure!(
        workers(&primary, first[&ids[0]], &pool).await?.len() == 2,
        "worker instances collapsed or leaked"
    );
    let (a, b) = tokio::join!(
        submit(
            &primary,
            first[&ids[0]],
            &pool,
            first[&ids[2]],
            "text.uppercase/v1",
            " Straße "
        ),
        submit(
            &primary,
            first[&ids[1]],
            &pool,
            first[&ids[2]],
            "text.uppercase/v1",
            "beamlet"
        ),
    );
    let a = completed(a?, first[&ids[2]], " STRASSE ")?;
    let b = completed(b?, first[&ids[2]], "BEAMLET")?;
    ensure!(a != b, "Authority operations collided");
    completed(
        submit(
            &primary,
            first[&ids[0]],
            &pool,
            first[&ids[3]],
            "text.reverse/v1",
            "abc",
        )
        .await?,
        first[&ids[3]],
        "cba",
    )?;
    ensure!(
        matches!(
            submit(
                &primary,
                first[&ids[0]],
                &pool,
                first[&ids[2]],
                "text.reverse/v1",
                "x"
            )
            .await?,
            Outcome::Rejected { .. }
        ),
        "disabled capability executed"
    );
    ensure!(
        matches!(
            submit(
                &primary,
                first[&ids[0]],
                &pool,
                first[&ids[5]],
                "text.reverse/v1",
                "x"
            )
            .await?,
            Outcome::Rejected { .. }
        ),
        "cross infrastructure target accepted"
    );
    let node_target = RoleInstance {
        role_id: node_id,
        session_id: first[&ids[3]].session_id,
    };
    ensure!(
        matches!(
            submit(
                &primary,
                first[&ids[0]],
                &pool,
                node_target,
                "text.reverse/v1",
                "x"
            )
            .await?,
            Outcome::Rejected { .. }
        ),
        "node UUID still routes tasks"
    );

    // Local stdin is read once; only the chosen Authority may create the operation.
    writeln!(
        cluster.processes[first_process]
            .child
            .stdin
            .as_mut()
            .context("node stdin")?,
        "/authority {}\n/pool blue\n/capability text.uppercase/v1\nlocal sample",
        ids[1]
    )?;
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        let log = fs::read_to_string(&cluster.processes[first_process].log)?;
        if log.contains("LOCAL SAMPLE") {
            break;
        }
        ensure!(
            Instant::now() < deadline,
            "local Authority selection did not execute"
        );
        sleep(Duration::from_millis(40)).await;
    }
    cluster.processes[first_process].child.stdin.take(); // EOF must leave all roles running.

    // One failed broker does not shut down unrelated infrastructure/roles.
    cluster.kill(primary_broker)?;
    sleep(Duration::from_millis(250)).await;
    completed(
        submit(
            &secondary,
            first[&ids[4]],
            &pool,
            first[&ids[5]],
            "text.reverse/v1",
            "isolated",
        )
        .await?,
        first[&ids[5]],
        "detalosi",
    )?;
    cluster.broker(primary_port, "primary-restarted")?;
    let primary = connect(primary_port).await?;
    let deadline = Instant::now() + Duration::from_secs(8);
    while discover(&primary, &pool).await?.len() != 2 {
        ensure!(Instant::now() < deadline, "roles did not reconnect");
        sleep(Duration::from_millis(60)).await;
    }
    wait_for_workers(
        &primary,
        first[&ids[0]],
        &pool,
        &[first[&ids[2]], first[&ids[3]]],
    )
    .await?;

    // A second Node may host these persistent roles with new sessions; old shutdown cannot remove them.
    let moved = source.replace(&node_id.to_string(), &Uuid::new_v4().to_string());
    let (second_process, second) = cluster.node("moved", &moved, 6).await?;
    for id in &ids {
        ensure!(
            first[id].session_id != second[id].session_id,
            "role restart reused session"
        );
    }
    wait_for_workers(
        &primary,
        second[&ids[0]],
        &pool,
        &[
            first[&ids[2]],
            second[&ids[2]],
            first[&ids[3]],
            second[&ids[3]],
        ],
    )
    .await?;
    ensure!(
        discover(&primary, &pool).await?.len() == 4,
        "role sessions collapsed"
    );
    completed(
        submit(
            &primary,
            second[&ids[0]],
            &pool,
            first[&ids[2]],
            "text.uppercase/v1",
            "old",
        )
        .await?,
        first[&ids[2]],
        "OLD",
    )?;
    completed(
        submit(
            &primary,
            first[&ids[0]],
            &pool,
            second[&ids[2]],
            "text.uppercase/v1",
            "new",
        )
        .await?,
        second[&ids[2]],
        "NEW",
    )?;
    cluster.stop(first_process).await?;
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let view = workers(&primary, second[&ids[0]], &pool).await?;
        if [ids[2], ids[3]].iter().all(|id| {
            view.iter().any(|item| {
                item.presence.instance == first[id] && item.presence.state == PresenceState::Offline
            })
        }) {
            break;
        }
        ensure!(
            Instant::now() < deadline,
            "not all Worker roles announced offline: {view:?}"
        );
        sleep(Duration::from_millis(40)).await;
    }
    ensure!(
        matches!(
            submit(
                &primary,
                second[&ids[0]],
                &pool,
                first[&ids[2]],
                "text.uppercase/v1",
                "old"
            )
            .await?,
            Outcome::Rejected { .. }
        ),
        "old session fell back to new one"
    );
    completed(
        submit(
            &primary,
            second[&ids[0]],
            &pool,
            second[&ids[2]],
            "text.uppercase/v1",
            "still here",
        )
        .await?,
        second[&ids[2]],
        "STILL HERE",
    )?;
    cluster.stop(second_process).await?;
    for process in [first_process, second_process] {
        let log = fs::read_to_string(&cluster.processes[process].log)?;
        for event in [
            "operation.received",
            "attempt.received",
            "report.created",
            "report.send_started",
            "report.sent",
            "worker.drained",
        ] {
            ensure!(
                log.contains(&format!("event=\"{event}\"")),
                "missing Worker event after crate extraction: {event}"
            );
        }
    }
    ensure!(
        fs::read_to_string(cluster.root.path().join("composed.ron"))? == source,
        "config rewritten"
    );
    ensure!(
        fs::read_to_string(cluster.root.path().join("moved.ron"))? == moved,
        "moved config rewritten"
    );
    println!(
        "PASS: multiple roles, infrastructure isolation, exact role routing, independent sessions, reconnect, local selection and shutdown"
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires nats-server; explicit configured deadline test"]
async fn configured_report_deadlines_and_input_capacity_are_independent_per_authority() -> Result<()>
{
    use beamlet_protocol::{ReportData, WorkerCall, WorkerReply};
    use tokio::time::timeout;
    let mut cluster = Cluster::new()?;
    let port = port()?;
    cluster.broker(port, "deadlines")?;
    let client = connect(port).await?;
    let ids = [Uuid::new_v4(), Uuid::new_v4()];
    let pool: Pool = "blue".parse()?;
    let fake_worker = RoleInstance {
        role_id: Uuid::new_v4(),
        session_id: Uuid::new_v4(),
    };
    let mut calls = client
        .subscribe(subject::worker(&pool, fake_worker))
        .await?;
    client.flush().await?;
    let base = authority(r#""blue""#, 30, 1).replace("input_capacity: 7", "input_capacity: 1");
    let source = config(
        Uuid::new_v4(),
        &format!(
            r#""local": nats(url: "nats://127.0.0.1:{port}", connect_timeout_secs: 9, subscription_capacity: 32)"#
        ),
        &[
            role(
                ids[0],
                "local",
                &base.replace("report_timeout_secs: 4", "report_timeout_secs: 1"),
            ),
            role(ids[1], "local", &base),
        ],
    );
    let (process, roles) = cluster.node("deadlines-node", &source, 2).await?;
    client
        .publish(
            subject::presence(&pool),
            encode(&Presence {
                pool: pool.clone(),
                instance: fake_worker,
                label: None,
                capabilities: BTreeSet::from(["text.reverse/v1".into()]),
                sequence: 1,
                state: PresenceState::Ready,
            })?
            .into(),
        )
        .await?;
    for authority in roles.values() {
        wait_for_workers(&client, *authority, &pool, &[fake_worker]).await?;
    }
    let mut pending = Vec::new();
    for (id, text) in [(ids[0], "short"), (ids[1], "long")] {
        let client = client.clone();
        let pool = pool.clone();
        let authority = roles[&id];
        pending.push(tokio::spawn(async move {
            submit(
                &client,
                authority,
                &pool,
                fake_worker,
                "text.reverse/v1",
                text,
            )
            .await
        }));
    }
    let mut received = BTreeMap::new();
    for _ in 0..2 {
        let message = timeout(Duration::from_secs(2), calls.next())
            .await?
            .context("call missing")?;
        let call: WorkerCall = decode(&message.payload)?;
        received.insert(call.task.text.clone(), (message, call));
    }
    // long Authority 已占用一个执行名额; 只能再排队一条输入.
    let queued = client.send_request(
        subject::authority(&pool, roles[&ids[1]]),
        async_nats::Request::new()
            .payload(
                encode(&Command::Submit {
                    request_id: Uuid::new_v4(),
                    authority: roles[&ids[1]],
                    submission: Submission {
                        pool: pool.clone(),
                        worker: Selection::Exact(fake_worker),
                        task: Task {
                            capability: "text.reverse/v1".into(),
                            text: "queued".into(),
                        },
                    },
                })?
                .into(),
            )
            .timeout(Some(Duration::from_secs(6))),
    );
    tokio::pin!(queued);
    ensure!(
        futures::poll!(&mut queued).is_pending(),
        "queued request unexpectedly completed"
    );
    client.flush().await?;
    // 同一连接按顺序发送; 控制查询的回复确认前一条提交已进入业务队列.
    workers(&client, roles[&ids[1]], &pool).await?;
    let overflow = timeout(
        Duration::from_secs(2),
        submit(
            &client,
            roles[&ids[1]],
            &pool,
            fake_worker,
            "text.reverse/v1",
            "overflow",
        ),
    )
    .await??;
    ensure!(
        matches!(overflow, Outcome::Rejected { .. }),
        "input_capacity=1 did not reject excess input"
    );
    let short = pending.remove(0);
    let outcome = timeout(Duration::from_secs(2), short).await???;
    ensure!(
        matches!(outcome, Outcome::Unresolved { .. }),
        "short deadline was ignored"
    );
    // 超过旧的固定 3 秒后, 新的 4 秒期限仍应接受报告.
    sleep(Duration::from_millis(2200)).await;
    let long = pending.pop().context("long request missing")?;
    ensure!(
        !long.is_finished(),
        "long deadline was truncated by an old timeout"
    );
    let (message, call) = received.remove("long").context("long call missing")?;
    client
        .publish(
            message.reply.context("reply missing")?,
            encode(&WorkerReply::Report {
                pool: pool.clone(),
                worker: fake_worker,
                report: ReportData {
                    report_id: "long-report".into(),
                    operation_id: call.operation_id,
                    attempt_id: call.attempt_id,
                    result: "long-done".into(),
                },
            })?
            .into(),
        )
        .await?;
    completed(long.await??, fake_worker, "long-done")?;
    let message = timeout(Duration::from_secs(2), calls.next())
        .await?
        .context("queued call missing")?;
    let call: WorkerCall = decode(&message.payload)?;
    ensure!(
        call.task.text == "queued",
        "unexpected retry or overflow delivery"
    );
    client
        .publish(
            message.reply.context("reply missing")?,
            encode(&WorkerReply::Report {
                pool,
                worker: fake_worker,
                report: ReportData {
                    report_id: "queued-report".into(),
                    operation_id: call.operation_id,
                    attempt_id: call.attempt_id,
                    result: "queued-done".into(),
                },
            })?
            .into(),
        )
        .await?;
    let reply: beamlet_protocol::Reply = decode(&queued.await?.payload)?;
    completed(reply.outcome, fake_worker, "queued-done")?;
    cluster.stop(process).await?;
    ensure!(
        fs::read_to_string(cluster.root.path().join("deadlines-node.ron"))? == source,
        "config rewritten"
    );
    Ok(())
}
