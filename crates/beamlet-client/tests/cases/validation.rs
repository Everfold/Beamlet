use crate::support::{self, instance};
use beamlet_api::{Command, Pool, Selection, Submission, Task, Uuid};
use beamlet_client::{BuildError, Client, Error, Options};
use std::time::Duration;

/// 验证 Client 参数检查无需执行器即可拒绝非法期限。
///
/// # 参数
///
/// 无。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
#[test]
fn construction_checks_durations_without_needing_an_executor() {
    for duration in [Duration::ZERO, Duration::MAX] {
        for options in [
            Options {
                request_timeout: duration,
                ..Options::default()
            },
            Options {
                discovery_window: duration,
                ..Options::default()
            },
        ] {
            let (transport, _) = support::pair::<String, String>();
            assert!(matches!(
                Client::new(transport, options),
                Err(BuildError::InvalidDuration(_))
            ));
        }
    }
    let (transport, _) = support::pair::<String, String>();
    assert!(matches!(
        Client::new(
            transport,
            Options {
                request_timeout: Duration::from_secs(1),
                discovery_window: Duration::from_secs(1)
            }
        ),
        Err(BuildError::DiscoveryWindowTooLong)
    ));
}
/// 验证非法命令在进入传输前被拒绝。
///
/// # 参数
///
/// 无。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
#[tokio::test]
async fn invalid_commands_are_rejected_before_transport() {
    let (transport, mut events) = support::pair::<String, String>();
    let client = Client::new(transport, Options::default()).unwrap();
    assert!(matches!(
        client.discover(&Default::default()).await,
        Err(Error::InvalidRequest(_))
    ));
    for (request_id, authority, worker, capability) in [
        (Uuid::nil(), instance(1), Selection::Any, "echo/v1"),
        (Uuid::new_v4(), instance(0), Selection::Any, "echo/v1"),
        (
            Uuid::new_v4(),
            instance(1),
            Selection::Exact(instance(0)),
            "echo/v1",
        ),
        (Uuid::new_v4(), instance(1), Selection::Any, ""),
    ] {
        assert!(matches!(
            client
                .request(Command::Submit {
                    request_id,
                    authority,
                    submission: Submission {
                        pool: Pool::default(),
                        worker,
                        task: Task {
                            capability: capability.into(),
                            payload: String::new()
                        }
                    },
                })
                .await,
            Err(Error::InvalidRequest(_))
        ));
    }
    assert!(
        events.try_recv().is_err(),
        "invalid commands reached transport"
    );
}
