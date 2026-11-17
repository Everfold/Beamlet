use beamlet_api::{
    AuthorityInbox, AuthorityInfo, ClientTransport, Command, DeliveryError, DiscoveryInbox,
    Outcome, Pool, Presence, PresenceChannel, PresenceState, RoleInstance, Selection, Submission,
    Task, Uuid, WorkerBinding, WorkerInbox,
};
use beamlet_authority::{Authority, Concurrency, Options};
use beamlet_primitives::Report;
use beamlet_transport_local::Local;
use std::{future::Future, time::Duration};
use tokio::{sync::oneshot, time::timeout};
const DEADLINE: Duration = Duration::from_secs(3);
fn instance() -> RoleInstance {
    RoleInstance {
        role_id: Uuid::new_v4(),
        session_id: Uuid::new_v4(),
    }
}
async fn within<T>(future: impl Future<Output = T>) -> T {
    timeout(DEADLINE, future)
        .await
        .expect("authority contract stalled")
}
fn info() -> AuthorityInfo {
    AuthorityInfo {
        instance: instance(),
        label: Some("authority".into()),
        pools: [Pool::default(), "other".parse().unwrap()].into(),
    }
}
fn submit(authority: RoleInstance, worker: Selection, text: &str) -> Command {
    Command::Submit {
        request_id: Uuid::new_v4(),
        authority,
        submission: Submission {
            pool: Pool::default(),
            worker,
            task: Task {
                capability: "echo/v1".into(),
                payload: text.into(),
            },
        },
    }
}
async fn workers(local: &Local, authority: RoleInstance) -> Vec<beamlet_api::WorkerView> {
    let reply = local
        .request(
            Command::Workers {
                request_id: Uuid::new_v4(),
                authority,
                pool: Pool::default(),
                capability: None,
            },
            DEADLINE,
        )
        .await
        .unwrap();
    let Outcome::Workers { workers } = reply.outcome else {
        panic!("workers expected");
    };
    workers
}
#[tokio::test]
async fn control_progresses_at_capacity_and_exact_never_falls_back() {
    let scenario = async {
        let local = Local::new(16).unwrap();
        let info = info();
        let endpoint = local.bind_authority(&info).unwrap();
        let authority = Authority::new(
            info.clone(),
            Options {
                concurrency: Concurrency::Limited(1),
                input_capacity: 1,
                ..Options::default()
            },
            endpoint.connection(),
        )
        .unwrap();
        let directory = authority.directory();
        let (stop, shutdown) = oneshot::channel();
        let running = tokio::spawn(authority.run(endpoint, local.clone(), shutdown));
        let target = instance();
        let binding = WorkerBinding {
            pool: Pool::default(),
            instance: target,
            capabilities: ["echo/v1".into()].into(),
        };
        let (mut worker, presence) = local.bind_worker(binding.clone()).unwrap();
        let mut snapshot = Presence {
            pool: Pool::default(),
            instance: target,
            label: Some("worker".into()),
            capabilities: binding.capabilities,
            sequence: 1,
            state: PresenceState::Ready,
        };
        presence.publish(&snapshot).await.unwrap();
        assert_eq!(workers(&local, info.instance).await.len(), 1);
        assert_eq!(
            directory.workers(&Pool::default(), None)[0]
                .presence
                .instance,
            target
        );
        // 第一份任务悬挂, 第二份排队, 第三份被队列拒绝; 控制查询不占业务槽位.
        let first = local.request(
            submit(info.instance, Selection::Exact(target), "first"),
            DEADLINE,
        );
        tokio::pin!(first);
        let incoming = tokio::select! {
            incoming = worker.receive() => incoming.unwrap().unwrap(),
            _ = &mut first => panic!("request finished before report"),
        };
        let second = local.request(submit(info.instance, Selection::Any, "queued"), DEADLINE);
        tokio::pin!(second);
        std::future::poll_fn(|cx| {
            assert!(second.as_mut().poll(cx).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        workers(&local, info.instance).await; // control has classified preceding submit
        let outcome = local
            .request(submit(info.instance, Selection::Any, "full"), DEADLINE)
            .await
            .unwrap()
            .outcome;
        assert!(matches!(outcome, Outcome::Rejected { .. }));
        let id = Uuid::new_v4();
        let mut discovered = local.discover(&info.pools, id).await.unwrap();
        assert_eq!(
            discovered
                .receive()
                .await
                .unwrap()
                .unwrap()
                .authority
                .instance,
            info.instance
        );
        snapshot.sequence += 1;
        snapshot.state = PresenceState::Offline;
        presence.publish(&snapshot).await.unwrap();
        assert_eq!(
            workers(&local, info.instance).await[0].presence.state,
            PresenceState::Offline
        );
        incoming
            .reply
            .send(Report::new(
                "report-1".into(),
                incoming.call.attempt,
                "done".into(),
            ))
            .await
            .unwrap();
        assert!(
            matches!(first.await.unwrap().outcome, Outcome::Completed { worker, result, .. } if worker == target && result == "done")
        );
        assert!(matches!(
            second.await.unwrap().outcome,
            Outcome::Rejected { .. }
        ));
        snapshot.sequence += 1;
        snapshot.state = PresenceState::Ready;
        presence.publish(&snapshot).await.unwrap();
        workers(&local, info.instance).await;
        let wrong_session = RoleInstance {
            session_id: Uuid::new_v4(),
            ..target
        };
        let outcome = local
            .request(
                submit(
                    info.instance,
                    Selection::Exact(wrong_session),
                    "old session",
                ),
                DEADLINE,
            )
            .await
            .unwrap()
            .outcome;
        assert!(matches!(outcome, Outcome::Rejected { .. }));
        // 关闭后明确无法交付仍返回 Unresolved, 不重新选择.
        worker.stop().await.unwrap();
        let outcome = local
            .request(
                submit(info.instance, Selection::Exact(target), "gone"),
                DEADLINE,
            )
            .await
            .unwrap()
            .outcome;
        assert!(
            matches!(outcome, Outcome::Unresolved { worker, reason, .. } if worker == target && reason == DeliveryError::NoReceiver.to_string())
        );
        stop.send(()).unwrap();
        running.await.unwrap().unwrap();
    };
    within(scenario).await;
}

#[derive(Clone)]
struct Numbers(Vec<u32>);
struct Sum(u32);
#[tokio::test]
async fn generic_authority_worker_and_client_use_public_interfaces_only() {
    let scenario = async {
        let local = Local::<Numbers, Sum>::new(8).unwrap();
        let info = info();
        let endpoint = local.bind_authority(&info).unwrap();
        let authority =
            Authority::new(info.clone(), Options::default(), endpoint.connection()).unwrap();
        let mut capabilities = beamlet_worker::Capabilities::new();
        capabilities
            .register("sum/v1", |input: Numbers, _| async move {
                Ok(Sum(input.0.iter().sum()))
            })
            .unwrap();
        let worker_id = instance();
        let worker = beamlet_worker::Worker::builder(worker_id, capabilities)
            .build()
            .unwrap();
        let (inbox, presence) = local.bind_worker(worker.binding()).unwrap();
        let (stop_worker, worker_stop) = oneshot::channel();
        let worker = tokio::spawn(worker.run(inbox, presence, worker_stop));
        let (stop_authority, authority_stop) = oneshot::channel();
        let authority = tokio::spawn(authority.run(endpoint, local.clone(), authority_stop));
        loop {
            let reply = local
                .request(
                    Command::Workers {
                        request_id: Uuid::new_v4(),
                        authority: info.instance,
                        pool: Pool::default(),
                        capability: None,
                    },
                    DEADLINE,
                )
                .await
                .unwrap();
            if matches!(reply.outcome, Outcome::Workers { workers } if !workers.is_empty()) {
                break;
            }
            tokio::task::yield_now().await;
        }
        let reply = local
            .request(
                Command::Submit {
                    request_id: Uuid::new_v4(),
                    authority: info.instance,
                    submission: Submission {
                        pool: Pool::default(),
                        worker: Selection::Exact(worker_id),
                        task: Task {
                            capability: "sum/v1".into(),
                            payload: Numbers(vec![2, 3, 5]),
                        },
                    },
                },
                DEADLINE,
            )
            .await
            .unwrap();
        let Outcome::Completed {
            worker: executed,
            result,
            operation_id,
            attempt_id,
            report_id,
        } = reply.outcome
        else {
            panic!("completed expected");
        };
        assert_eq!(executed, worker_id);
        assert_eq!(result.0, 10);
        assert!(operation_id.starts_with(&info.instance.session_id.to_string()));
        assert!(attempt_id.starts_with(&info.instance.session_id.to_string()));
        assert!(report_id.starts_with(&worker_id.session_id.to_string()));
        stop_worker.send(()).unwrap();
        worker.await.unwrap().unwrap();
        stop_authority.send(()).unwrap();
        authority.await.unwrap().unwrap();
    };
    within(scenario).await;
}

struct LocalRequests(tokio::sync::mpsc::Receiver<beamlet_authority::Request>);
impl beamlet_authority::LocalInbox for LocalRequests {
    async fn receive(
        &mut self,
    ) -> Result<Option<beamlet_authority::Request>, beamlet_api::TransportError> {
        Ok(self.0.recv().await)
    }
    async fn stop(&mut self) -> Result<(), beamlet_api::TransportError> {
        self.0.close();
        while self.0.try_recv().is_ok() {}
        Ok(())
    }
}

#[tokio::test]
async fn local_eof_leaves_remote_requests_running() {
    let scenario = async {
        let local = Local::new(8).unwrap();
        let info = info();
        let endpoint = local.bind_authority(&info).unwrap();
        let authority =
            Authority::new(info.clone(), Options::default(), endpoint.connection()).unwrap();
        let (send, receive) = tokio::sync::mpsc::channel(1);
        let (reply, result) = oneshot::channel();
        let Command::Submit { submission, .. } = submit(info.instance, Selection::Any, "local")
        else {
            unreachable!();
        };
        send.send(beamlet_authority::Request {
            submission,
            reply: beamlet_api::Responder::new(move |outcome| async move {
                reply
                    .send(outcome)
                    .map_err(|_| beamlet_api::ReplyError::new("caller closed"))
            }),
        })
        .await
        .unwrap();
        drop(send);
        let (stop, shutdown) = oneshot::channel();
        let running = tokio::spawn(authority.run_with_local(
            endpoint,
            local.clone(),
            LocalRequests(receive),
            shutdown,
        ));
        assert!(matches!(result.await.unwrap(), Outcome::Rejected { .. }));
        let remote = local
            .request(submit(info.instance, Selection::Any, "remote"), DEADLINE)
            .await
            .unwrap();
        assert!(matches!(remote.outcome, Outcome::Rejected { .. }));
        stop.send(()).unwrap();
        running.await.unwrap().unwrap();
    };
    within(scenario).await;
}

#[derive(Clone)]
struct PendingDelivery {
    started: std::sync::Arc<tokio::sync::Notify>,
    dropped: std::sync::Arc<tokio::sync::Notify>,
}
struct DropNotice(std::sync::Arc<tokio::sync::Notify>);
impl Drop for DropNotice {
    fn drop(&mut self) {
        self.0.notify_one();
    }
}
impl beamlet_api::Delivery for PendingDelivery {
    async fn deliver(
        &self,
        _call: beamlet_api::Call,
        _deadline: Duration,
    ) -> Result<beamlet_api::WorkerReport, DeliveryError> {
        let _notice = DropNotice(self.dropped.clone());
        self.started.notify_one();
        std::future::pending().await
    }
}

#[tokio::test]
async fn cancelling_authority_releases_delivery_and_control_endpoint() {
    let scenario = async {
        let local = Local::new(8).unwrap();
        let info = info();
        let endpoint = local.bind_authority(&info).unwrap();
        let authority =
            Authority::new(info.clone(), Options::default(), endpoint.connection()).unwrap();
        let delivery = PendingDelivery {
            started: Default::default(),
            dropped: Default::default(),
        };
        let running = tokio::spawn(authority.run(
            endpoint,
            delivery.clone(),
            std::future::pending::<std::io::Result<()>>(),
        ));
        let target = instance();
        let binding = WorkerBinding {
            pool: Pool::default(),
            instance: target,
            capabilities: ["echo/v1".into()].into(),
        };
        let (_inbox, presence) = local.bind_worker(binding.clone()).unwrap();
        presence
            .publish(&Presence {
                pool: binding.pool,
                instance: target,
                label: None,
                capabilities: binding.capabilities,
                sequence: 1,
                state: PresenceState::Ready,
            })
            .await
            .unwrap();
        assert_eq!(workers(&local, info.instance).await.len(), 1);
        let request = local.request(
            submit(info.instance, Selection::Exact(target), "pending"),
            DEADLINE,
        );
        tokio::pin!(request);
        tokio::select! {
            () = delivery.started.notified() => {},
            _ = &mut request => panic!("pending delivery completed"),
        }
        running.abort();
        assert!(running.await.unwrap_err().is_cancelled());
        delivery.dropped.notified().await;
        assert!(
            request.await.is_err(),
            "cancelled authority retained the reply"
        );
        loop {
            if local.bind_authority(&info).is_ok() {
                break;
            }
            tokio::task::yield_now().await;
        }
    };
    within(scenario).await;
}
