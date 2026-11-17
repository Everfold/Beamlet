use beamlet_api::{AuthorityInfo, Connection, ConnectionView, Pool, RoleInstance, Uuid};
use beamlet_authority::{Authority, BuildError, Concurrency, Options};
use std::time::Duration;

fn build(info: AuthorityInfo, options: Options) -> Result<Authority, BuildError> {
    Authority::new(
        info,
        options,
        ConnectionView::new(|| Connection {
            epoch: 1,
            connected: true,
        }),
    )
}
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
