use beamlet_api::{AuthorityInfo, Pool, RoleInstance, Uuid};
use beamlet_authority::{Authority, BuildError, Concurrency, Options};
use std::time::Duration;

/// 用指定配置测试 Authority 的构造.
fn build(info: AuthorityInfo, options: Options) -> Result<Authority, BuildError> {
    Authority::new(info, options)
}
/// 构造 Authority 测试用身份与 pool 描述.
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
