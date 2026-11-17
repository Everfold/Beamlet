use beamlet_api::{RoleInstance, Uuid};
use beamlet_worker::{BuildError, Capabilities, CapabilityError, Concurrency, Options, Worker};
use std::time::Duration;
/// 为测试生成独立的角色 UUID 和会话 UUID。
///
/// # 返回值
///
/// 新的 RoleInstance，用于隔离测试绑定。
fn instance() -> RoleInstance {
    RoleInstance {
        role_id: Uuid::new_v4(),
        session_id: Uuid::new_v4(),
    }
}
/// 构造带测试处理器的能力注册表。
///
/// # 返回值
///
/// 注册完成的 Capabilities；注册失败时 panic。
fn capabilities() -> Capabilities {
    let mut capabilities = Capabilities::new();
    capabilities
        .register("echo/v1", |text, _| async { Ok(text) })
        .unwrap();
    capabilities
        .register("other/v1", |text, _| async { Ok(text) })
        .unwrap();
    capabilities
}
/// 验证注册拒绝重复能力，构建 Worker 后只保留已知启用处理器。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
#[test]
fn registry_rejects_duplicates_and_worker_freezes_only_known_enabled_handlers() {
    let mut registered = capabilities();
    assert!(matches!(
        registered.register("echo/v1", |_, _| async {
            panic!("duplicate must not replace")
        }),
        Err(CapabilityError::Duplicate(_))
    ));
    assert!(matches!(
        registered.register("", |text, _| async { Ok(text) }),
        Err(CapabilityError::Invalid { .. })
    ));
    let worker = Worker::builder(instance(), registered)
        .enabled(["echo/v1", "echo/v1"])
        .build()
        .unwrap();
    assert_eq!(worker.binding().capabilities, ["echo/v1".into()].into());
    assert!(matches!(
        Worker::builder(instance(), capabilities())
            .enabled(["missing/v1"])
            .build(),
        Err(BuildError::Capabilities(CapabilityError::Unknown(_)))
    ));
    assert!(matches!(
        Worker::builder(instance(), capabilities())
            .enabled(Vec::<String>::new())
            .build(),
        Err(BuildError::Capabilities(CapabilityError::Empty))
    ));
    assert!(matches!(
        Worker::builder(instance(), Capabilities::<String, String>::new()).build(),
        Err(BuildError::Capabilities(CapabilityError::Empty))
    ));
}
/// 验证直接构造 Worker 时在绑定传输前拒绝非法参数与 nil 身份。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
#[test]
fn invalid_direct_options_and_nil_identity_are_rejected_before_transport_binding() {
    for options in [
        Options {
            concurrency: Concurrency::Limited(0),
            ..Options::default()
        },
        Options {
            heartbeat_interval: Duration::ZERO,
            ..Options::default()
        },
        Options {
            heartbeat_interval: Duration::MAX,
            ..Options::default()
        },
        Options {
            announcement_timeout: Duration::ZERO,
            ..Options::default()
        },
        Options {
            announcement_timeout: Duration::MAX,
            ..Options::default()
        },
    ] {
        assert!(
            Worker::builder(instance(), capabilities())
                .options(options)
                .build()
                .is_err()
        );
    }
    assert!(
        Worker::builder(instance(), capabilities())
            .options(Options {
                concurrency: Concurrency::Unlimited,
                ..Options::default()
            })
            .build()
            .is_ok()
    );
    for identity in [
        RoleInstance {
            role_id: Uuid::nil(),
            ..instance()
        },
        RoleInstance {
            session_id: Uuid::nil(),
            ..instance()
        },
    ] {
        assert!(matches!(
            Worker::builder(identity, capabilities()).build(),
            Err(BuildError::InvalidIdentity)
        ));
    }
}
