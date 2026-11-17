use beamlet_api::{AuthorityInfo, Pool, RoleInstance, Uuid};
use beamlet_authority::{Authority, BuildError, Concurrency, Options};
use std::time::Duration;

/// 用始终连接的视图尝试构造 Authority.
///
/// # 参数
///
/// - `info`: Authority 的角色/会话身份, 标签及其加入的 pools.
/// - `options`: 当前模块的运行参数; 期限使用 Duration, 容量/并发由对应字段约束.
///
/// # 返回值
///
/// 构造成功返回 Authority; 参数或身份非法时返回 BuildError.
fn build(info: AuthorityInfo, options: Options) -> Result<Authority, BuildError> {
    Authority::new(info, options)
}
/// 构造 Authority 测试用身份与 pool 描述.
///
/// # 返回值
///
/// 带有效独立身份和非空 pools 的 AuthorityInfo.
fn info() -> AuthorityInfo {
    AuthorityInfo {
        instance: RoleInstance {
            role_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
        },
        label: None,
        pools: [Pool::default()].into(),
    }
}

/// 验证直接构造 Authority 时拒绝非法身份, 容量及期限.
///
/// # 返回值
///
/// `()`; 全部断言通过时正常结束, 断言失败或未预期错误会使测试失败.
#[test]
fn rejects_invalid_identity_and_parameters_before_runtime_resources_are_created() {
    for instance in [
        RoleInstance {
            role_id: Uuid::nil(),
            session_id: Uuid::new_v4(),
        },
        RoleInstance {
            role_id: Uuid::new_v4(),
            session_id: Uuid::nil(),
        },
    ] {
        assert!(matches!(
            build(AuthorityInfo { instance, ..info() }, Options::default()),
            Err(BuildError::InvalidIdentity)
        ));
    }
    assert!(matches!(
        build(
            AuthorityInfo {
                pools: Default::default(),
                ..info()
            },
            Options::default()
        ),
        Err(BuildError::EmptyPools)
    ));
    assert!(matches!(
        build(
            info(),
            Options {
                concurrency: Concurrency::Limited(0),
                ..Options::default()
            }
        ),
        Err(BuildError::ZeroConcurrency)
    ));
    assert!(
        build(
            info(),
            Options {
                concurrency: Concurrency::Unlimited,
                ..Options::default()
            }
        )
        .is_ok()
    );
    for value in [0, usize::MAX] {
        for (options, field) in [
            (
                Options {
                    input_capacity: value,
                    ..Options::default()
                },
                "input_capacity",
            ),
            (
                Options {
                    max_concurrent_replies: value,
                    ..Options::default()
                },
                "max_concurrent_replies",
            ),
        ] {
            assert!(
                matches!(build(info(), options), Err(BuildError::InvalidCapacity(name)) if name == field)
            );
        }
    }
    for value in [Duration::ZERO, Duration::MAX] {
        for (options, field) in [
            (
                Options {
                    worker_timeout: value,
                    ..Options::default()
                },
                "worker_timeout",
            ),
            (
                Options {
                    report_timeout: value,
                    ..Options::default()
                },
                "report_timeout",
            ),
        ] {
            assert!(
                matches!(build(info(), options), Err(BuildError::InvalidDuration(name)) if name == field)
            );
        }
    }
}
