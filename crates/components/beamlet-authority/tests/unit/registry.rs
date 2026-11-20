use super::*;
use beamlet_api::Connection;
use beamlet_api::Uuid;
use tokio::sync::watch;

/// 验证租约按 pool/会话隔离, 公告序号单调, 重连后必须重新续租.
#[test]
fn leases_are_scoped_monotonic_and_require_fresh_heartbeats_after_reconnect() {
    let pool = Pool::default();
    let (connection, receiver) = watch::channel(Connection {
        epoch: 1,
        connected: true,
    });
    let registry = Registry::new(BTreeSet::from([pool.clone()]), Duration::from_secs(15));
    let source = Source {
        id: 0,
        connection: ConnectionView::new(move || *receiver.borrow()),
    };
    let now = Instant::now();
    let mut heartbeat = Presence {
        pool: pool.clone(),
        instance: RoleInstance {
            role_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
        },
        label: None,
        capabilities: BTreeSet::from(["text.reverse/v1".into()]),
        sequence: 1,
        state: PresenceState::Ready,
    };
    assert!(registry.observe(&source, heartbeat.clone(), 1, now));
    let select = |time| registry.select(&pool, Selection::Any, "text.reverse/v1", time);
    assert_eq!(select(now), Some((heartbeat.instance, 0)));
    assert!(!registry.observe(&source, heartbeat.clone(), 1, now + Duration::from_secs(14)));
    assert_eq!(select(now + Duration::from_secs(15)), None);
    heartbeat.sequence += 1;
    assert!(registry.observe(&source, heartbeat.clone(), 1, now + Duration::from_secs(16)));
    connection.send_replace(Connection {
        epoch: 2,
        connected: false,
    });
    assert_eq!(select(now), None);
    connection.send_replace(Connection {
        epoch: 3,
        connected: true,
    });
    assert_eq!(select(now), None);
    assert!(!registry.observe(&source, heartbeat.clone(), 3, now));
    heartbeat.sequence += 1;
    assert!(registry.observe(&source, heartbeat.clone(), 3, now));
    let old = heartbeat.clone();
    heartbeat.instance.session_id = Uuid::new_v4();
    heartbeat.sequence = 1;
    assert!(registry.observe(&source, heartbeat.clone(), 3, now));
    let mut offline = old;
    offline.sequence += 1;
    offline.state = PresenceState::Offline;
    assert!(registry.observe(&source, offline, 3, now));
    assert_eq!(select(now), Some((heartbeat.instance, 0)));
    assert_eq!(
        registry.select(&pool, Selection::Any, "text.reverse/v2", now),
        None
    );
    heartbeat.state = PresenceState::Draining;
    heartbeat.sequence += 1;
    assert!(registry.observe(&source, heartbeat.clone(), 3, now));
    assert_eq!(select(now), None);
    registry.expire(now + Duration::from_secs(60));
    assert!(registry.workers(&pool, None, now).is_empty());
    heartbeat.pool = "other".parse().unwrap();
    assert!(!registry.observe(&source, heartbeat, 3, now));
}

/// 验证 Selection::Any 对同池同能力 Worker 轮询分发，并在节点退场或过期时平滑跳过；Selection::Exact 不回退。
#[test]
fn select_any_round_robins_ready_workers_and_exact_never_falls_back() {
    let pool = Pool::default();
    let now = Instant::now();
    let registry = Registry::new([pool.clone()].into(), Duration::from_secs(15));
    let source = Source {
        id: 0,
        connection: ConnectionView::new(|| Connection {
            epoch: 1,
            connected: true,
        }),
    };
    let workers: Vec<RoleInstance> = (1..=3_u128)
        .map(|id| RoleInstance {
            role_id: Uuid::from_u128(id),
            session_id: Uuid::from_u128(id + 100),
        })
        .collect();
    let mut snapshots: Vec<Presence> = workers
        .iter()
        .map(|&instance| Presence {
            pool: pool.clone(),
            instance,
            label: None,
            capabilities: ["echo/v1".into()].into(),
            sequence: 1,
            state: PresenceState::Ready,
        })
        .collect();
    for snapshot in &snapshots {
        assert!(registry.observe(&source, snapshot.clone(), 1, now));
    }

    // 三个就绪 Worker 连续 6 次 Selection::Any 应按轮询顺序各命中两次。
    let picks: Vec<_> = (0..6)
        .map(|_| {
            registry
                .select(&pool, Selection::Any, "echo/v1", now)
                .unwrap()
                .0
        })
        .collect();
    assert_eq!(
        picks,
        vec![
            workers[0], workers[1], workers[2], workers[0], workers[1], workers[2],
        ]
    );

    // Exact 精确锁定第二个 Worker。
    assert_eq!(
        registry.select(&pool, Selection::Exact(workers[1]), "echo/v1", now),
        Some((workers[1], 0))
    );

    // 第二个 Worker 变为 Draining：Exact 必须直接返回 None，Any 在剩余两个间继续轮询。
    snapshots[1].sequence = 2;
    snapshots[1].state = PresenceState::Draining;
    assert!(registry.observe(&source, snapshots[1].clone(), 1, now));
    assert_eq!(
        registry.select(&pool, Selection::Exact(workers[1]), "echo/v1", now),
        None
    );
    let remaining: Vec<_> = (0..4)
        .map(|_| {
            registry
                .select(&pool, Selection::Any, "echo/v1", now)
                .unwrap()
                .0
        })
        .collect();
    assert_eq!(
        remaining,
        vec![workers[0], workers[2], workers[0], workers[2]]
    );

    // 第一个 Worker 仅在 t=0 更新，第三个 Worker 在 t=10s 续租；t=15s 时仅剩第三个 Worker 有效。
    snapshots[2].sequence = 2;
    assert!(registry.observe(
        &source,
        snapshots[2].clone(),
        1,
        now + Duration::from_secs(10)
    ));
    let later = now + Duration::from_secs(15);
    assert_eq!(
        registry.select(&pool, Selection::Exact(workers[0]), "echo/v1", later),
        None
    );
    assert_eq!(
        registry.select(&pool, Selection::Any, "echo/v1", later),
        Some((workers[2], 0))
    );

    // 全部过期后 Any 与 Exact 均安全返回 None。
    let expired = now + Duration::from_secs(30);
    assert_eq!(
        registry.select(&pool, Selection::Any, "echo/v1", expired),
        None
    );
    assert_eq!(
        registry.select(&pool, Selection::Exact(workers[2]), "echo/v1", expired),
        None
    );
}

/// 同会话跨传输去重, 断线仅使所属租约失效, 重连必须接收新心跳.
#[test]
fn transports_have_independent_epochs_and_duplicates_do_not_renew_leases() {
    let pool = Pool::default();
    let now = Instant::now();
    let registry = Registry::new([pool.clone()].into(), Duration::from_secs(15));
    let (link, view) = watch::channel(Connection {
        epoch: 1,
        connected: true,
    });
    let remote = Source {
        id: 0,
        connection: ConnectionView::new(move || *view.borrow()),
    };
    let local = Source {
        id: 1,
        connection: ConnectionView::new(|| Connection {
            epoch: 1,
            connected: true,
        }),
    };
    let mut heartbeat = Presence {
        pool: pool.clone(),
        instance: RoleInstance {
            role_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
        },
        label: None,
        capabilities: ["echo/v1".into()].into(),
        sequence: 1,
        state: PresenceState::Ready,
    };
    assert!(registry.observe(&remote, heartbeat.clone(), 1, now));
    assert!(registry.observe(&local, heartbeat.clone(), 1, now + Duration::from_secs(5)));
    assert!(registry.workers(&pool, None, now + Duration::from_secs(15))[0].stale);
    assert_eq!(registry.workers(&pool, None, now).len(), 1);
    let select = || registry.select(&pool, Selection::Exact(heartbeat.instance), "echo/v1", now);
    assert_eq!(select(), Some((heartbeat.instance, 0)));
    link.send_replace(Connection {
        epoch: 2,
        connected: false,
    });
    assert_eq!(select(), Some((heartbeat.instance, 1)));
    assert!(!registry.workers(&pool, None, now)[0].stale);
    link.send_replace(Connection {
        epoch: 3,
        connected: true,
    });
    assert!(!registry.observe(&remote, heartbeat.clone(), 1, now));
    assert!(!registry.observe(&remote, heartbeat.clone(), 3, now));
    assert_eq!(select(), Some((heartbeat.instance, 1)));
    heartbeat.sequence = 2;
    assert!(registry.observe(&remote, heartbeat.clone(), 3, now));
    assert_eq!(
        registry.select(&pool, Selection::Any, "echo/v1", now),
        Some((heartbeat.instance, 0))
    );
    heartbeat.state = PresenceState::Draining;
    heartbeat.sequence = 3;
    assert!(registry.observe(&remote, heartbeat.clone(), 3, now));
    assert_eq!(registry.select(&pool, Selection::Any, "echo/v1", now), None);
    heartbeat.state = PresenceState::Ready;
    assert!(!registry.observe(&local, heartbeat.clone(), 1, now));
    heartbeat.sequence = 2;
    assert!(!registry.observe(&local, heartbeat, 1, now));
    assert_eq!(registry.select(&pool, Selection::Any, "echo/v1", now), None);
}
