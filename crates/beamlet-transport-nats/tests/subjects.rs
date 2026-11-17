use beamlet_api::{Pool, RoleInstance, Uuid};
use beamlet_transport_nats::subject;

/// 验证非法 pool 不能突破主题片段隔离, 不同会话路由不同.
///
/// # 返回值
///
/// `()`; 全部断言通过时正常结束, 断言失败或未预期错误会使测试失败.
#[test]
fn pools_cannot_escape_subject_tokens_and_sessions_route_separately() {
    for invalid in ["", "a.b", "a*", ">", "a b", " a", "\n", "池"] {
        assert!(invalid.parse::<Pool>().is_err(), "{invalid:?}");
    }
    let a: Pool = "A_1-test".parse().unwrap();
    let b: Pool = "a_1-test".parse().unwrap();
    assert_ne!(a, b);
    let instance = RoleInstance {
        role_id: Uuid::new_v4(),
        session_id: Uuid::new_v4(),
    };
    let restarted = RoleInstance {
        session_id: Uuid::new_v4(),
        ..instance
    };
    assert_ne!(
        subject::worker(&a, instance),
        subject::worker(&a, restarted)
    );
    assert_ne!(subject::worker(&a, instance), subject::worker(&b, instance));
    assert_ne!(
        subject::worker(&a, instance),
        subject::authority(&a, instance)
    );
}
