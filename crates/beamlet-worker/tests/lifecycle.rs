use beamlet_api::{
    Call, Connection, Incoming, Pool, Presence, PresenceChannel, PresenceEvent, PresenceState,
    Responder, RoleInstance, TransportError, Uuid, WorkerInbox,
};
use beamlet_primitives::Attempt;
use beamlet_worker::{Capabilities, Concurrency, Options, RunError, Worker};
use std::{
    future::Future,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::{
    sync::{mpsc, oneshot, watch},
    time::timeout,
};

/// Worker 生命周期测试的步骤等待期限：3 秒。
const DEADLINE: Duration = Duration::from_secs(3);
/// 限制异步测试步骤的等待时长。
///
/// # 参数
///
/// - `f`：需要限制等待时长的异步测试步骤。
///
/// # 返回值
///
/// 原 future 的输出；未在测试期限内完成时 panic。
async fn within<T>(f: impl Future<Output = T>) -> T {
    timeout(DEADLINE, f)
        .await
        .expect("worker lifecycle stalled")
}
/// 构造 Worker 生命周期测试复用的固定角色实例。
///
/// # 返回值
///
/// 角色 UUID 数值为 3、会话 UUID 数值为 4 的 RoleInstance。
fn instance() -> RoleInstance {
    RoleInstance {
        role_id: Uuid::from_u128(3),
        session_id: Uuid::from_u128(4),
    }
}
/// 按测试序号创建关联身份明确的 Worker 调用。
///
/// # 参数
///
/// - `index`：测试调用序号，用于关联 Operation、Attempt 和业务数据。
///
/// # 返回值
///
/// 供指定测试 Worker 执行的 Call。
fn call(index: usize) -> Call {
    Call {
        pool: Pool::default(),
        authority: RoleInstance {
            role_id: Uuid::from_u128(1),
            session_id: Uuid::from_u128(2),
        },
        worker: instance(),
        attempt: Attempt::from_parts(format!("operation-{index}"), format!("attempt-{index}")),
        capability: "echo/v1".into(),
        payload: format!("payload-{index}"),
    }
}
/// 把测试调用队列适配为 WorkerInbox 的接收端。
struct Inbox(
    /// 可以被关闭并继续排空的 Incoming 队列。
    mpsc::Receiver<Incoming>,
);
impl WorkerInbox for Inbox {
    /// 接收测试队列中下一份 Worker 调用。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Inbox` 的可变引用。
    ///
    /// # 返回值
    ///
    /// 有调用为 `Ok(Some(Incoming))`；关闭且排空后为 `Ok(None)`。
    async fn receive(&mut self) -> Result<Option<Incoming>, TransportError> {
        Ok(self.0.recv().await)
    }
    /// 停止测试 Worker 队列接收新调用。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Inbox` 的可变引用。
    ///
    /// # 返回值
    ///
    /// 返回 `Ok(())`，已排队的调用仍可被消费。
    async fn stop(&mut self) -> Result<(), TransportError> {
        self.0.close();
        Ok(())
    }
}
/// 可控制连接、探测和刷新阻塞的公告通道替身。
struct Announcements {
    /// 把 Worker 发布的完整公告转给测试观察者的通道。
    snapshots: mpsc::UnboundedSender<Presence>,
    /// 从测试驱动读取模拟连接状态的接收端。
    link: watch::Receiver<Connection>,
    /// 接收测试触发的探测或连接变化事件。
    events: mpsc::Receiver<PresenceEvent>,
    /// 为 true 时让 flush 挂起，验证公告期限与取消清理。
    block_flush: Arc<AtomicBool>,
}
impl PresenceChannel for Announcements {
    /// 读取测试驱动的模拟连接状态。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Announcements` 的共享引用。
    ///
    /// # 返回值
    ///
    /// 当前 watch 中的 Connection 副本。
    fn connection(&self) -> Connection {
        *self.link.borrow()
    }
    /// 读取测试驱动发送的探测或连接事件。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Announcements` 的可变引用。
    ///
    /// # 返回值
    ///
    /// 成功返回 PresenceEvent；事件入口关闭返回 TransportError。
    async fn receive(&mut self) -> Result<PresenceEvent, TransportError> {
        self.events
            .recv()
            .await
            .ok_or_else(|| TransportError::new("probe channel closed"))
    }
    /// 把 Worker 完整公告转发给测试观察者。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Announcements` 的共享引用。
    /// - `presence`：Worker 的完整状态快照，包括 pool、角色会话、能力、序号和运行状态。
    ///
    /// # 返回值
    ///
    /// 观察者接收成功返回 `Ok(())`；观察队列关闭返回 TransportError。
    async fn publish(&self, presence: &Presence) -> Result<(), TransportError> {
        self.snapshots
            .send(presence.clone())
            .map_err(TransportError::new)
    }
    /// 按测试开关模拟立即完成或永久挂起的传输刷新。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Announcements` 的共享引用。
    ///
    /// # 返回值
    ///
    /// 未阻塞时为 `Ok(())`；阻塞模式不主动完成，需要由上层期限或取消结束等待。
    async fn flush(&self) -> Result<(), TransportError> {
        if self.block_flush.load(Ordering::SeqCst) {
            std::future::pending::<()>().await;
        }
        Ok(())
    }
}
/// Worker 生命周期测试的输入、公告、连接与停止控制集合。
struct Harness {
    /// 向 Worker 提供调用的有界发送端。
    input: mpsc::Sender<Incoming>,
    /// 读取 Worker 发布的全部完整公告。
    snapshots: mpsc::UnboundedReceiver<Presence>,
    /// 供测试修改连接世代与在线状态的发送端。
    link: watch::Sender<Connection>,
    /// 供测试触发探测或连接事件的发送端。
    events: mpsc::Sender<PresenceEvent>,
    /// 为 true 时让 flush 挂起，验证公告期限与取消清理。
    block_flush: Arc<AtomicBool>,
    /// 向运行中的 Worker 请求停止的一次性发送端。
    stop: oneshot::Sender<()>,
    /// Worker 主循环的异步任务句柄，供测试等待或取消。
    task: tokio::task::JoinHandle<Result<(), RunError>>,
}
/// 给 Worker 装配可控输入、连接和公告通道并启动运行。
///
/// # 参数
///
/// - `worker`：已校验构造完成、即将接入可控通道的 Worker。
///
/// # 返回值
///
/// Harness，供测试发送调用、模拟连接/探测、观察公告及触发停止。
fn start(worker: Worker) -> Harness {
    let (input, inbox) = mpsc::channel(8);
    let (snapshot_send, snapshots) = mpsc::unbounded_channel();
    let (link, connection) = watch::channel(Connection {
        epoch: 1,
        connected: true,
    });
    let (events, probes) = mpsc::channel(8);
    let block_flush = Arc::new(AtomicBool::new(false));
    let channel = Announcements {
        snapshots: snapshot_send,
        link: connection,
        events: probes,
        block_flush: block_flush.clone(),
    };
    let (stop, signal) = oneshot::channel();
    let task = tokio::spawn(worker.run(Inbox(inbox), channel, signal));
    Harness {
        input,
        snapshots,
        link,
        events,
        block_flush,
        stop,
        task,
    }
}
/// 持续读取公告，直到出现指定 Worker 运行状态。
///
/// # 参数
///
/// - `h`：Worker 生命周期测试的控制与观察句柄。
/// - `state`：期望下一次观察到的 Worker 公告状态。
///
/// # 返回值
///
/// 第一份匹配状态的 Presence；等待超时或公告队列关闭时 panic。
async fn next(h: &mut Harness, state: PresenceState) -> Presence {
    loop {
        let snapshot = within(h.snapshots.recv()).await.unwrap();
        if snapshot.state == state {
            return snapshot;
        }
    }
}

/// 验证执行挂起期间心跳、探测、重连和 Draining 公告仍推进。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
#[tokio::test]
async fn heartbeats_probe_reconnect_and_draining_progress_while_execution_is_suspended() {
    let mut capabilities = Capabilities::new();
    let (started, began) = oneshot::channel();
    let (release, wait) = oneshot::channel();
    let handoff = std::sync::Mutex::new(Some((started, wait)));
    capabilities
        .register("echo/v1", move |payload, context| {
            let (started, wait) = handoff.lock().unwrap().take().unwrap();
            async move {
                assert_eq!(context.operation_id(), "operation-1");
                assert_eq!(context.attempt_id(), "attempt-1");
                assert_eq!(context.authority(), call(1).authority);
                assert_eq!(context.worker(), instance());
                assert_eq!(context.pool(), &Pool::default());
                started.send(()).unwrap();
                wait.await.unwrap();
                Ok(payload)
            }
        })
        .unwrap();
    let options = Options {
        heartbeat_interval: Duration::from_millis(20),
        ..Options::default()
    };
    let worker = Worker::builder(instance(), capabilities)
        .label("blocked worker")
        .options(options)
        .build()
        .unwrap();
    let mut h = start(worker);
    let ready = next(&mut h, PresenceState::Ready).await;
    assert_eq!(ready.instance, instance());
    assert_eq!(ready.label.as_deref(), Some("blocked worker"));
    assert_eq!(ready.capabilities, ["echo/v1".into()].into());
    let (report, response) = oneshot::channel();
    h.input
        .send(Incoming {
            call: call(1),
            reply: Responder::new(move |value| async move {
                report.send(value).unwrap();
                Ok(())
            }),
        })
        .await
        .unwrap();
    within(began).await.unwrap();
    let heartbeat = next(&mut h, PresenceState::Ready).await;
    assert!(heartbeat.sequence > ready.sequence);
    h.events.send(PresenceEvent::Probe).await.unwrap();
    let probed = next(&mut h, PresenceState::Ready).await;
    assert!(probed.sequence > heartbeat.sequence);
    let disconnected = Connection {
        epoch: 2,
        connected: false,
    };
    h.link.send_replace(disconnected);
    h.events
        .send(PresenceEvent::Connection(disconnected))
        .await
        .unwrap();
    while h.snapshots.try_recv().is_ok() {}
    assert!(
        timeout(Duration::from_millis(60), h.snapshots.recv())
            .await
            .is_err()
    );
    let connected = Connection {
        epoch: 3,
        connected: true,
    };
    h.link.send_replace(connected);
    h.events
        .send(PresenceEvent::Connection(connected))
        .await
        .unwrap();
    let resumed = next(&mut h, PresenceState::Ready).await;
    assert!(resumed.sequence > probed.sequence);
    h.stop.send(()).unwrap();
    // Only a pending task keeps the role in draining, so offline cannot mask the transition.
    let draining = loop {
        let snapshot = within(h.snapshots.recv()).await.unwrap();
        if snapshot.state == PresenceState::Draining {
            break snapshot;
        }
    };
    assert!(!h.task.is_finished());
    assert!(draining.sequence > resumed.sequence);
    release.send(()).unwrap();
    let report = within(response).await.unwrap();
    assert_eq!(report.id(), &format!("{}:report:1", instance().session_id));
    assert_eq!(report.result(), "payload-1");
    let offline = loop {
        let snapshot = within(h.snapshots.recv()).await.unwrap();
        if snapshot.state == PresenceState::Offline {
            break snapshot;
        }
    };
    assert!(offline.sequence > draining.sequence);
    within(h.task).await.unwrap().unwrap();
}

/// 验证禁用能力和错误目标不会执行，报告发送等待仍占用并发名额。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
#[tokio::test]
async fn disabled_or_misrouted_calls_never_execute_and_reply_waiting_occupies_capacity() {
    let mut capabilities = Capabilities::new();
    let (executed, mut observed) = mpsc::unbounded_channel();
    capabilities
        .register("echo/v1", move |payload, _| {
            let executed = executed.clone();
            async move {
                executed.send(()).unwrap();
                Ok(payload)
            }
        })
        .unwrap();
    assert!(
        capabilities
            .register("echo/v1", |_, _| async {
                panic!("duplicate handler installed")
            })
            .is_err()
    );
    capabilities
        .register("disabled/v1", |_, _| async {
            panic!("disabled handler ran")
        })
        .unwrap();
    let mut h = start(
        Worker::builder(instance(), capabilities)
            .enabled(["echo/v1"])
            .build()
            .unwrap(),
    );
    let ready = next(&mut h, PresenceState::Ready).await;
    assert_eq!(ready.capabilities, ["echo/v1".into()].into());
    for invalid in [
        Call {
            capability: "disabled/v1".into(),
            ..call(0)
        },
        Call {
            pool: "other".parse().unwrap(),
            ..call(0)
        },
        Call {
            worker: call(0).authority,
            ..call(0)
        },
    ] {
        let (reply, gone) = oneshot::channel();
        h.input
            .send(Incoming {
                call: invalid,
                reply: Responder::new(move |_| async move {
                    reply.send(()).unwrap();
                    Ok(())
                }),
            })
            .await
            .unwrap();
        assert!(
            within(gone).await.is_err(),
            "rejected call produced a report"
        );
        assert!(observed.try_recv().is_err());
    }
    let (release, wait) = oneshot::channel();
    let (sending, started) = oneshot::channel();
    h.input
        .send(Incoming {
            call: call(1),
            reply: Responder::new(move |_| async move {
                sending.send(()).unwrap();
                wait.await.unwrap();
                Ok(())
            }),
        })
        .await
        .unwrap();
    within(started).await.unwrap();
    within(observed.recv()).await.unwrap();
    let (reply, report) = oneshot::channel();
    h.input
        .send(Incoming {
            call: call(2),
            reply: Responder::new(move |value| async move {
                reply.send(value).unwrap();
                Ok(())
            }),
        })
        .await
        .unwrap();
    assert!(
        timeout(Duration::from_millis(30), observed.recv())
            .await
            .is_err()
    );
    release.send(()).unwrap();
    within(observed.recv()).await.unwrap();
    assert_eq!(within(report).await.unwrap().attempt_id(), "attempt-2");
    h.stop.send(()).unwrap();
    within(h.task).await.unwrap().unwrap();
}

/// 验证取消 Worker 运行会释放异步处理器并关闭调用入口。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
#[tokio::test]
async fn dropping_run_releases_suspended_handler_and_closes_input() {
    let mut capabilities = Capabilities::new();
    let (started, began) = oneshot::channel();
    let (finished, dropped) = oneshot::channel::<()>();
    let handoff = std::sync::Mutex::new(Some((started, finished)));
    capabilities
        .register("echo/v1", move |payload, _| {
            let (started, finished) = handoff.lock().unwrap().take().unwrap();
            async move {
                let _guard = finished;
                started.send(()).unwrap();
                std::future::pending::<()>().await;
                Ok(payload)
            }
        })
        .unwrap();
    let h = start(Worker::builder(instance(), capabilities).build().unwrap());
    h.input
        .send(Incoming {
            call: call(1),
            reply: Responder::new(|_| async { panic!("cancelled task replied") }),
        })
        .await
        .unwrap();
    within(began).await.unwrap();
    h.task.abort();
    assert!(within(h.task).await.unwrap_err().is_cancelled());
    assert!(within(dropped).await.is_err());
    assert!(h.input.is_closed());
}

/// 验证离线刷新受到期限限制，并保留停止信号的原始错误。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
#[tokio::test]
async fn offline_flush_is_bounded_and_shutdown_source_is_preserved() {
    let mut capabilities = Capabilities::new();
    capabilities
        .register("echo/v1", |text: String, _| async { Ok(text) })
        .unwrap();
    let worker = Worker::builder(instance(), capabilities)
        .options(Options {
            concurrency: Concurrency::Unlimited,
            announcement_timeout: Duration::from_millis(20),
            ..Options::default()
        })
        .build()
        .unwrap();
    let mut h = start(worker);
    next(&mut h, PresenceState::Ready).await;
    h.block_flush.store(true, Ordering::SeqCst);
    h.stop.send(()).unwrap();
    assert!(matches!(
        within(h.task).await.unwrap(),
        Err(RunError::AnnouncementTimeout)
    ));

    let mut capabilities = Capabilities::new();
    capabilities
        .register("echo/v1", |text: String, _| async { Ok(text) })
        .unwrap();
    let h = start(Worker::builder(instance(), capabilities).build().unwrap());
    drop(h.stop);
    let error = within(h.task).await.unwrap().unwrap_err();
    assert!(matches!(error, RunError::Shutdown(_)));
    assert!(std::error::Error::source(&error).is_some());
}
