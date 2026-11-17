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
    let registry = Registry::new(
        BTreeSet::from([pool.clone()]),
        Duration::from_secs(15),
        ConnectionView::new(move || *receiver.borrow()),
    );
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
    assert!(registry.observe(heartbeat.clone(), 1, now));
    let select = |time| registry.select(&pool, Selection::Any, "text.reverse/v1", time);
    assert_eq!(select(now), Some(heartbeat.instance));
    assert!(!registry.observe(heartbeat.clone(), 1, now + Duration::from_secs(14)));
    assert_eq!(select(now + Duration::from_secs(15)), None);
    heartbeat.sequence += 1;
    assert!(registry.observe(heartbeat.clone(), 1, now + Duration::from_secs(16)));
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
    assert!(!registry.observe(heartbeat.clone(), 3, now));
    heartbeat.sequence += 1;
    assert!(registry.observe(heartbeat.clone(), 3, now));
    let old = heartbeat.clone();
    heartbeat.instance.session_id = Uuid::new_v4();
    heartbeat.sequence = 1;
    assert!(registry.observe(heartbeat.clone(), 3, now));
    let mut offline = old;
    offline.sequence += 1;
    offline.state = PresenceState::Offline;
    assert!(registry.observe(offline, 3, now));
    assert_eq!(select(now), Some(heartbeat.instance));
    assert_eq!(
        registry.select(&pool, Selection::Any, "text.reverse/v2", now),
        None
    );
    heartbeat.state = PresenceState::Draining;
    heartbeat.sequence += 1;
    assert!(registry.observe(heartbeat.clone(), 3, now));
    assert_eq!(select(now), None);
    registry.expire(now + Duration::from_secs(60));
    assert!(registry.workers(&pool, None, now).is_empty());
    heartbeat.pool = "other".parse().unwrap();
    assert!(!registry.observe(heartbeat, 3, now));
}
