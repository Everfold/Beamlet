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

const DEADLINE: Duration = Duration::from_secs(3);
async fn within<T>(f: impl Future<Output = T>) -> T {
    timeout(DEADLINE, f)
        .await
        .expect("worker lifecycle stalled")
}
fn instance() -> RoleInstance {
    RoleInstance {
        role_id: Uuid::from_u128(3),
        session_id: Uuid::from_u128(4),
    }
}
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
struct Inbox(mpsc::Receiver<Incoming>);
impl WorkerInbox for Inbox {
    async fn receive(&mut self) -> Result<Option<Incoming>, TransportError> {
        Ok(self.0.recv().await)
    }
    async fn stop(&mut self) -> Result<(), TransportError> {
        self.0.close();
        Ok(())
    }
}
struct Announcements {
    snapshots: mpsc::UnboundedSender<Presence>,
    link: watch::Receiver<Connection>,
    events: mpsc::Receiver<PresenceEvent>,
    block_flush: Arc<AtomicBool>,
}
impl PresenceChannel for Announcements {
    fn connection(&self) -> Connection {
        *self.link.borrow()
    }
    async fn receive(&mut self) -> Result<PresenceEvent, TransportError> {
        self.events
            .recv()
            .await
            .ok_or_else(|| TransportError::new("probe channel closed"))
    }
    async fn publish(&self, presence: &Presence) -> Result<(), TransportError> {
        self.snapshots
            .send(presence.clone())
            .map_err(TransportError::new)
    }
    async fn flush(&self) -> Result<(), TransportError> {
        if self.block_flush.load(Ordering::SeqCst) {
            std::future::pending::<()>().await;
        }
        Ok(())
    }
}
struct Harness {
    input: mpsc::Sender<Incoming>,
    snapshots: mpsc::UnboundedReceiver<Presence>,
    link: watch::Sender<Connection>,
    events: mpsc::Sender<PresenceEvent>,
    block_flush: Arc<AtomicBool>,
    stop: oneshot::Sender<()>,
    task: tokio::task::JoinHandle<Result<(), RunError>>,
}
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
async fn next(h: &mut Harness, state: PresenceState) -> Presence {
    loop {
        let snapshot = within(h.snapshots.recv()).await.unwrap();
        if snapshot.state == state {
            return snapshot;
        }
    }
}

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
