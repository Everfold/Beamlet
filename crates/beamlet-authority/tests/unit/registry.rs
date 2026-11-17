use super::*;
use beamlet_api::Connection;
use beamlet_api::Uuid;
use tokio::sync::watch;

/// 验证租约按 pool/会话隔离, 公告序号单调, 重连后必须重新续租.
///
/// # 返回值
///
/// `()`; 全部断言通过时正常结束, 断言失败或未预期错误会使测试失败.
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
    assert!(registry.observe(&local, heartbeat.clone(), 1, now));
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
    heartbeat.sequence = 2;
    heartbeat.state = PresenceState::Ready;
    assert!(!registry.observe(&local, heartbeat, 1, now));
    assert_eq!(registry.select(&pool, Selection::Any, "echo/v1", now), None);
}
