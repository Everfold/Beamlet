use beamlet_api::{
    AuthorityInfo, ClientTransport, Command, Delivery, DeliveryError, Outcome, Pool, Presence,
    PresenceChannel, PresenceState, RoleInstance, Selection, Submission, Task, Uuid, WorkerBinding,
    WorkerInbox, WorkerReport,
};
use beamlet_authority::{Authority, Concurrency, Options, Transports};
use beamlet_primitives::Report;
use beamlet_transport_local::Local;
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::sync::oneshot;

/// 所有测试交付的等待期限.
const DEADLINE: Duration = Duration::from_secs(2);

/// 创建独立的测试角色会话.
fn instance() -> RoleInstance {
    RoleInstance {
        role_id: Uuid::new_v4(),
        session_id: Uuid::new_v4(),
    }
}

/// 构造默认 pool 的 Authority 描述.
fn info() -> AuthorityInfo {
    AuthorityInfo {
        instance: instance(),
        label: None,
        pools: [Pool::default()].into(),
    }
}

/// 构造定向提交给指定 Worker 的命令.
fn submit(authority: RoleInstance, worker: RoleInstance, text: &str) -> Command {
    Command::Submit {
        request_id: Uuid::new_v4(),
        authority,
        submission: Submission {
            pool: Pool::default(),
            worker: Selection::Exact(worker),
            task: Task {
                capability: "echo/v1".into(),
                payload: text.into(),
            },
        },
    }
}

/// 在本地传输上绑定并公告测试 Worker.
async fn worker(local: &Local, target: RoleInstance) -> beamlet_transport_local::WorkerEndpoints {
    let binding = WorkerBinding {
        pool: Pool::default(),
        instance: target,
        capabilities: ["echo/v1".into()].into(),
    };
    let (inbox, presence) = local.bind_worker(binding.clone()).unwrap();
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
    (inbox, presence)
}

/// 同一 Authority 从任意入口查询并交付另一消息空间的 Worker, 共用签发序列.
#[tokio::test]
async fn one_authority_routes_between_independent_spaces_and_releases_all_endpoints() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let spaces = [Local::new(8).unwrap(), Local::new(8).unwrap()];
        let info = info();
        let authority = Authority::new(info.clone(), Options { concurrency: Concurrency::Limited(1), ..Options::default() }).unwrap();
        let directory = authority.directory();
        let mut transports = Transports::new();
        for space in &spaces { transports.add(space.bind_authority(&info).unwrap(), space.clone()); }
        let workers = [instance(), instance()];
        let (mut left, _left_presence) = worker(&spaces[0], workers[0]).await;
        let (mut right, _right_presence) = worker(&spaces[1], workers[1]).await;
        let (stop, shutdown) = oneshot::channel();
        let running = tokio::spawn(authority.run(transports, shutdown));
        while directory.workers(&Pool::default(), None).len() != 2 { tokio::task::yield_now().await; }
        for space in &spaces {
            let reply = space.request(Command::Workers { request_id: Uuid::new_v4(), authority: info.instance, pool: Pool::default(), capability: None }, DEADLINE).await.unwrap();
            assert!(matches!(reply.outcome, Outcome::Workers { workers } if workers.len() == 2));
        }
        for (index, inbox) in [&mut right, &mut left].into_iter().enumerate() {
            let command = submit(info.instance, workers[1 - index], "cross-space");
            let request = spaces[index].request(command, DEADLINE);
            tokio::pin!(request);
            let incoming = tokio::select! { incoming = inbox.receive() => incoming.unwrap().unwrap(), _ = &mut request => panic!("request completed before execution") };
            assert_eq!(incoming.call.worker, workers[1 - index]);
            assert_eq!(incoming.call.authority, info.instance);
            incoming.reply.send(Report::new(format!("report-{index}"), incoming.call.attempt, incoming.call.payload)).await.unwrap();
            let Outcome::Completed { operation_id, worker, result, .. } = request.await.unwrap().outcome else { panic!("expected completion") };
            assert_eq!(worker, workers[1 - index]);
            assert_eq!(result, "cross-space");
            assert_eq!(operation_id, format!("{}:operation:{}", info.instance.session_id, index + 1));
        }
        stop.send(()).unwrap();
        running.await.unwrap().unwrap();
        for space in &spaces {
            loop {
                if space.bind_authority(&info).is_ok() { break; }
                tokio::task::yield_now().await;
            }
        }
    }).await.expect("multi-transport test timed out");
}

/// 记录交付次数并返回未知结果的出口.
#[derive(Clone)]
struct Unknown(
    /// 实际交付调用次数.
    Arc<AtomicUsize>,
);
impl Delivery for Unknown {
    /// 记录调用次数并模拟交付结果未知.
    async fn deliver(
        &self,
        _call: beamlet_api::Call,
        _deadline: Duration,
    ) -> Result<WorkerReport, DeliveryError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(DeliveryError::Unknown("report lost".into()))
    }
}

/// 同会话的多个出口不会触发重试, 空集合在运行前拒绝.
#[tokio::test]
async fn unknown_delivery_never_retries_another_transport() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let spaces = [Local::new(8).unwrap(), Local::new(8).unwrap()];
        let info = info();
        let authority = Authority::new(info.clone(), Options::default()).unwrap();
        let target = instance();
        let counts = [Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0))];
        let mut transports = Transports::new();
        let mut workers = Vec::new();
        for (space, count) in spaces.iter().zip(&counts) {
            transports.add(space.bind_authority(&info).unwrap(), Unknown(count.clone()));
            workers.push(worker(space, target).await);
        }
        let (stop, shutdown) = oneshot::channel();
        let running = tokio::spawn(authority.run(transports, shutdown));
        // 查询与公告在同一入口按序处理, 确保两个出口的公告均已接收.
        for space in &spaces {
            let reply = space
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
            assert!(matches!(reply.outcome, Outcome::Workers { workers } if workers.len() == 1));
        }
        let reply = spaces[1]
            .request(submit(info.instance, target, "once"), DEADLINE)
            .await
            .unwrap();
        assert!(matches!(reply.outcome, Outcome::Unresolved { .. }));
        assert_eq!(counts[0].load(Ordering::SeqCst), 1);
        assert_eq!(counts[1].load(Ordering::SeqCst), 0);
        stop.send(()).unwrap();
        running.await.unwrap().unwrap();
        let authority = Authority::new(info, Options::default()).unwrap();
        let result = authority
            .run(
                Transports::<String, String>::new(),
                std::future::pending::<std::io::Result<()>>(),
            )
            .await;
        assert!(matches!(
            result,
            Err(beamlet_authority::RunError::NoTransports)
        ));
    })
    .await
    .expect("routing failure test timed out");
}
