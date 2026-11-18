//! 两个适配器共同执行的公开接口契约, 不依赖任何具体后端.
use beamlet_api::{
    AuthorityEvent, AuthorityInbox, AuthorityInfo, Call, ClientTransport, Command, Delivery,
    DeliveryError, DiscoveryInbox, DiscoveryReply, Outcome, Pool, Presence, PresenceChannel,
    PresenceEvent, PresenceState, Reply, RoleInstance, Uuid, WorkerBinding, WorkerInbox,
};
use beamlet_primitives::{Attempt, Report};
use std::{future::Future, time::Duration};
use tokio::time::timeout;

/// 单次契约交互的最长等待时间.
const DEADLINE: Duration = Duration::from_secs(2);

/// 以 base 和 base + 1 构造并返回固定角色会话.
fn instance(base: u128) -> RoleInstance {
    RoleInstance {
        role_id: Uuid::from_u128(base),
        session_id: Uuid::from_u128(base + 1),
    }
}
/// 返回启用 echo/v1 的测试 Worker 绑定.
pub fn binding() -> WorkerBinding {
    WorkerBinding {
        pool: Pool::default(),
        instance: instance(3),
        capabilities: ["echo/v1".into()].into(),
    }
}
/// 返回默认 pool 的测试 Authority.
pub fn authority() -> AuthorityInfo {
    AuthorityInfo {
        instance: instance(1),
        label: Some("authority".into()),
        pools: [Pool::default()].into(),
    }
}
/// 用 index 构造并返回一次可区分的调用.
fn call(index: usize) -> Call {
    Call {
        pool: Pool::default(),
        authority: instance(1),
        worker: instance(3),
        attempt: Attempt::from_parts(format!("operation-{index}"), format!("attempt-{index}")),
        capability: "echo/v1".into(),
        payload: format!("输入 {index}"),
    }
}
/// 在期限内等待 future 并返回其结果, 超时则使测试失败.
async fn within<T>(future: impl Future<Output = T>) -> T {
    timeout(DEADLINE, future).await.expect("transport stalled")
}

/// 通过 transport 交付并从 inbox 接收, 验证路由, 报告关联和排空.
pub async fn delivery(transport: impl Delivery, mut inbox: impl WorkerInbox) {
    // 多个在途请求按相反顺序回复, 验证每次调用独占回复入口.
    let first = call(1);
    let second = call(2);
    let (results, ()) = within(async {
        tokio::join!(
            async {
                tokio::join!(
                    transport.deliver(first, DEADLINE),
                    transport.deliver(second, DEADLINE)
                )
            },
            async {
                let a = inbox.receive().await.unwrap().unwrap();
                let b = inbox.receive().await.unwrap().unwrap();
                b.reply
                    .send(Report::new(
                        "report-b".into(),
                        b.call.attempt,
                        b.call.payload,
                    ))
                    .await
                    .unwrap();
                a.reply
                    .send(Report::new(
                        "report-a".into(),
                        a.call.attempt,
                        a.call.payload,
                    ))
                    .await
                    .unwrap();
            }
        )
    })
    .await;
    for (result, index) in [results.0, results.1].into_iter().zip([1, 2]) {
        let report = result.unwrap();
        assert_eq!(report.attempt_id(), &format!("attempt-{index}"));
        assert_eq!(report.result(), &format!("输入 {index}"));
    }
    // Exact session, pool 和 role 均独立路由, 不回退.
    for wrong in [
        RoleInstance {
            session_id: Uuid::new_v4(),
            ..instance(3)
        },
        instance(30),
    ] {
        let request = Call {
            worker: wrong,
            ..call(3)
        };
        assert_eq!(
            within(transport.deliver(request, DEADLINE))
                .await
                .unwrap_err(),
            DeliveryError::NoReceiver
        );
    }
    assert_eq!(
        within(transport.deliver(
            Call {
                pool: "other".parse().unwrap(),
                ..call(3)
            },
            DEADLINE
        ))
        .await
        .unwrap_err(),
        DeliveryError::NoReceiver
    );
    // 取消一次空 receive 后, 下一个有效调用仍可收取.
    assert!(
        timeout(Duration::from_millis(10), inbox.receive())
            .await
            .is_err()
    );
    // 对不在许可集合内的能力, 两个后端均明确拒绝且不交给执行器.
    let rejected = transport.deliver(
        Call {
            capability: "disabled/v1".into(),
            ..call(4)
        },
        DEADLINE,
    );
    let result = within(async {
        tokio::select! {
            result = rejected => result,
            received = inbox.receive() => panic!("unauthorized call reached worker: {received:?}"),
        }
    })
    .await;
    assert!(matches!(result, Err(DeliveryError::NotDelivered(_))));
    // 关联错误或空报告身份都不能成为完成结果.
    for invalid in [false, true] {
        let (result, ()) = within(async {
            tokio::join!(transport.deliver(call(5), DEADLINE), async {
                let incoming = inbox.receive().await.unwrap().unwrap();
                let report = if invalid {
                    Report::new(String::new(), incoming.call.attempt, "invalid".into())
                } else {
                    Report::new("report".into(), call(100).attempt, "invalid".into())
                };
                incoming.reply.send(report).await.unwrap();
            })
        })
        .await;
        assert!(matches!(result, Err(DeliveryError::Unknown(_))));
    }
    // 等待超时不撤销任务, 不重发. 超时后仍能取得调用并继续执行.
    let timed_out = transport.deliver(call(6), Duration::from_millis(30)).await;
    assert!(matches!(timed_out, Err(DeliveryError::Unknown(_))));
    let pending = within(inbox.receive()).await.unwrap().unwrap();
    assert_eq!(pending.call.attempt.id(), "attempt-6");
    assert!(
        timeout(Duration::from_millis(30), inbox.receive())
            .await
            .is_err()
    );
    drop(pending);
    // 已缓冲的调用在 stop 后仍可排空, 新调用不再进入.
    assert!(matches!(
        transport.deliver(call(7), Duration::from_millis(30)).await,
        Err(DeliveryError::Unknown(_))
    ));
    inbox.stop().await.unwrap();
    assert_eq!(
        within(inbox.receive())
            .await
            .unwrap()
            .unwrap()
            .call
            .attempt
            .id(),
        "attempt-7"
    );
    assert!(within(inbox.receive()).await.unwrap().is_none());
    assert_eq!(
        within(transport.deliver(call(8), DEADLINE))
            .await
            .unwrap_err(),
        DeliveryError::NoReceiver
    );
}

/// 从 endpoint 读取并返回下一个非连接事件.
async fn next_event(endpoint: &mut impl AuthorityInbox) -> AuthorityEvent {
    loop {
        match within(endpoint.receive()).await.unwrap().unwrap() {
            AuthorityEvent::Connection(_) => continue,
            event => return event,
        }
    }
}

/// 用 transport, endpoint 和 presence 验证发现, 查询, 探测和公告.
pub async fn control(
    transport: impl ClientTransport,
    mut endpoint: impl AuthorityInbox,
    mut presence: impl PresenceChannel,
) {
    let info = authority();
    let request_id = Uuid::new_v4();
    let mut replies = transport.discover(&info.pools, request_id).await.unwrap();
    let AuthorityEvent::Discovery {
        pool,
        request_id: received,
        reply,
    } = next_event(&mut endpoint).await
    else {
        panic!("discovery expected");
    };
    assert_eq!(pool, Pool::default());
    assert_eq!(received, request_id);
    reply
        .send(DiscoveryReply {
            request_id,
            authority: info.clone(),
        })
        .await
        .unwrap();
    let discovered = within(replies.receive()).await.unwrap().unwrap();
    assert_eq!(discovered.request_id, request_id);
    assert_eq!(discovered.authority.instance, info.instance);
    let command = Command::Workers {
        request_id,
        authority: info.instance,
        pool: Pool::default(),
        capability: Some("echo/v1".into()),
    };
    let (response, ()) = within(async {
        tokio::join!(transport.request(command, DEADLINE), async {
            let AuthorityEvent::Request {
                pool,
                command,
                reply,
            } = next_event(&mut endpoint).await
            else {
                panic!("request expected");
            };
            assert_eq!(pool, Pool::default());
            assert_eq!(command.authority(), info.instance);
            reply
                .send(Reply {
                    request_id: command.request_id(),
                    authority: info.instance,
                    outcome: Outcome::Workers { workers: vec![] },
                })
                .await
                .unwrap();
        })
    })
    .await;
    let response = response.unwrap();
    assert_eq!(response.authority, info.instance);
    assert_eq!(response.request_id, request_id);
    assert!(matches!(response.outcome, Outcome::Workers { workers } if workers.is_empty()));
    endpoint.probe().await.unwrap();
    loop {
        if matches!(
            within(presence.receive()).await.unwrap(),
            PresenceEvent::Probe
        ) {
            break;
        }
    }
    let snapshot = Presence {
        pool: Pool::default(),
        instance: instance(3),
        label: Some("worker".into()),
        capabilities: binding().capabilities,
        sequence: 27,
        state: PresenceState::Draining,
    };
    presence.publish(&snapshot).await.unwrap();
    presence.flush().await.unwrap();
    let AuthorityEvent::Presence {
        presence: received,
        epoch,
    } = next_event(&mut endpoint).await
    else {
        panic!("presence expected");
    };
    assert_eq!(received, snapshot);
    assert_eq!(epoch, endpoint.connection().current().epoch);
    let invalid = Presence {
        instance: instance(300),
        ..snapshot
    };
    assert!(presence.publish(&invalid).await.is_err());
}
