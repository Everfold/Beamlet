use super::*;
use beamlet_api::Presence;
use std::time::Duration;
use tokio::sync::oneshot;

/// 给异步测试步骤施加整体等待期限, 防止回归时永久挂起.
///
/// # 参数
///
/// - `future`: 需要在测试期限内完成的异步步骤.
///
/// # 返回值
///
/// future 的原始输出; 超过测试期限会 panic 并使测试失败.
async fn within<F: Future>(future: F) -> F::Output {
    tokio::time::timeout(Duration::from_secs(5), future)
        .await
        .expect("console async test timed out")
}
/// 用给定数值构造可重复的测试角色身份.
///
/// # 参数
///
/// - `id`: 确定性角色 UUID 的数值; 会话身份使用该值加 100.
///
/// # 返回值
///
/// 角色 UUID 使用 id, 会话 UUID 使用 id 加 100 的 RoleInstance.
fn instance(id: u128) -> RoleInstance {
    RoleInstance {
        role_id: Uuid::from_u128(id),
        session_id: Uuid::from_u128(id + 100),
    }
}
/// 构造供终端选择测试使用的 Authority 列表.
///
/// # 返回值
///
/// 两个身份不同, 加入默认 pool 的 AuthorityInfo.
fn authorities() -> Vec<AuthorityInfo> {
    [1, 2]
        .into_iter()
        .map(|id| AuthorityInfo {
            instance: instance(id),
            label: None,
            pools: BTreeSet::from([Pool::default()]),
        })
        .collect()
}
/// 构造供会话选择测试使用的 Worker 快照.
///
/// # 返回值
///
/// 含预设 Ready Worker 和能力集合的 `Vec<WorkerView>`.
fn workers() -> Vec<WorkerView> {
    vec![WorkerView {
        presence: Presence {
            pool: Pool::default(),
            instance: instance(3),
            label: None,
            capabilities: BTreeSet::from(["text.reverse/v1".into()]),
            sequence: 1,
            state: PresenceState::Ready,
        },
        last_seen_ms: 0,
        stale: false,
    }]
}
/// 创建已经选择 Authority, pool, 能力和 Worker 的会话.
///
/// # 返回值
///
/// 完成测试选择步骤的 Session; 准备失败会使测试失败.
fn selected() -> Session {
    let mut session = Session::new(
        BTreeSet::from([Pool::default()]),
        Settings::default().client.max_in_flight_requests,
    );
    session.discovered(authorities());
    session.line("/authority 1".into()).unwrap();
    session.line("/pool default".into()).unwrap();
    session.update_workers(workers());
    session.line("/capability 1".into()).unwrap();
    session.line("/worker 1".into()).unwrap();
    session
}
/// 把测试网络动作和结果组合为会话完成事件.
///
/// # 参数
///
/// - `action`: 尚未执行的终端网络动作.
/// - `result`: 模拟网络调用得到的 Outcome 或错误.
///
/// # 返回值
///
/// 与请求类型相符的 Completion; 传入发现动作会 panic.
fn response(action: Action, result: Result<Outcome>) -> Completion {
    let Action::Request {
        command,
        choice_revision,
    } = action
    else {
        panic!("expected request");
    };
    Completion::reply(command, choice_revision, result)
}
/// 构造带预设关联身份的已完成提交结果.
///
/// # 参数
///
/// - `text`: 作为成功业务结果展示的测试文本.
///
/// # 返回值
///
/// 携带输入文本作为业务结果的 Outcome::Completed.
fn completed(text: &str) -> Outcome {
    Outcome::Completed {
        operation_id: text.into(),
        attempt_id: format!("{text}:attempt"),
        report_id: format!("{text}:report"),
        worker: instance(3),
        result: text.into(),
    }
}
/// 为当前会话创建查询并记录它是最新一次 Worker 请求.
///
/// # 参数
///
/// - `session`: 需要更新的交互会话, 包括选择, 列表和最新查询身份.
///
/// # 返回值
///
/// 创建的查询 Action, 可用于随后构造完成事件.
fn track_query(session: &mut Session) -> Action {
    let query = session.workers().unwrap();
    session.workers_request = Some(query.request_id());
    query
}

/// 验证提交锁定目标快照, 旧拒绝不会覆盖新选择.
///
/// # 返回值
///
/// `()`; 全部断言通过时正常结束, 断言失败或未预期错误会使测试失败.
#[test]
fn submission_keeps_target_snapshot_and_late_rejection_preserves_new_choice() {
    let mut session = selected();
    let action = session.line("first".into()).unwrap().unwrap();
    let Action::Request {
        command:
            Command::Submit {
                authority,
                submission,
                ..
            },
        ..
    } = &action
    else {
        panic!("expected submission");
    };
    assert_eq!(*authority, instance(1));
    assert_eq!(submission.worker, Selection::Exact(instance(3)));
    assert_eq!(submission.task.payload, "first");
    // 即使重新选择同一个 Worker, 旧拒绝也不能撤销这次明确选择.
    session.line("/worker 1".into()).unwrap();
    session
        .complete(response(
            action,
            Ok(Outcome::Rejected {
                reason: "old rejection".into(),
            }),
        ))
        .unwrap();
    assert_eq!(session.choice.worker, Some(Selection::Exact(instance(3))));
    let current = session.line("second".into()).unwrap().unwrap();
    session
        .complete(response(
            current,
            Ok(Outcome::Rejected {
                reason: "current rejection".into(),
            }),
        ))
        .unwrap();
    assert_eq!(session.choice.worker, None);
}

/// 验证较旧的查询或发现完成事件不能替换新结果.
///
/// # 返回值
///
/// `()`; 全部断言通过时正常结束, 断言失败或未预期错误会使测试失败.
#[test]
fn older_queries_and_discovery_never_replace_newer_results() {
    let mut session = selected();
    let old = track_query(&mut session);
    let new = track_query(&mut session);
    session
        .complete(response(new, Ok(Outcome::Workers { workers: workers() })))
        .unwrap();
    session
        .complete(response(old, Ok(Outcome::Workers { workers: vec![] })))
        .unwrap();
    assert_eq!(session.workers.len(), 1);
    let before_selection = track_query(&mut session);
    session.line("/worker 1".into()).unwrap();
    session
        .complete(response(
            before_selection,
            Ok(Outcome::Workers { workers: vec![] }),
        ))
        .unwrap();
    assert_eq!(session.choice.worker, Some(Selection::Exact(instance(3))));
    assert_eq!(session.workers.len(), 1);
    let old_target = track_query(&mut session);
    session.line("/authority 2".into()).unwrap();
    session
        .complete(response(
            old_target,
            Ok(Outcome::Workers { workers: workers() }),
        ))
        .unwrap();
    assert_eq!(session.choice.authority, Some(instance(2)));
    assert!(session.workers.is_empty());

    let old = Uuid::new_v4();
    let new = Uuid::new_v4();
    session.discovery_request = Some(new);
    session
        .complete(Completion::Discovered {
            request_id: new,
            result: Ok(authorities()),
        })
        .unwrap();
    session
        .complete(Completion::Discovered {
            request_id: old,
            result: Ok(vec![]),
        })
        .unwrap();
    assert_eq!(session.authorities.len(), 2);
    assert_eq!(session.choice.authority, Some(instance(2)));
}

/// 验证等待中的提交不阻塞查询, EOF 后按完成顺序排空.
///
/// # 返回值
///
/// `()`; 全部断言通过时正常结束, 断言失败或未预期错误会使测试失败.
#[tokio::test]
async fn pending_submissions_allow_queries_and_eof_drains_out_of_order_results() {
    within(async {
        let mut session = selected();
        let (input, receive) = mpsc::channel(8);
        let (events, mut pending) = mpsc::unbounded_channel();
        let running = tokio::spawn(async move {
            drive(&mut session, receive, move |action: Action| {
                let events = events.clone();
                async move {
                    let (send, receive) = oneshot::channel();
                    events
                        .send((action, send))
                        .unwrap_or_else(|_| panic!("test observer closed"));
                    receive.await.unwrap()
                }
            })
            .await
            .unwrap();
            session
        });
        let (discovery, reply) = pending.recv().await.unwrap();
        reply
            .send(Completion::Discovered {
                request_id: discovery.request_id(),
                result: Ok(authorities()),
            })
            .unwrap_or_else(|_| panic!("discovery receiver closed"));
        input.send(Ok("slow".into())).await.unwrap();
        input.send(Ok("fast".into())).await.unwrap();
        input.send(Ok("/workers".into())).await.unwrap();
        let mut submissions = BTreeSet::new();
        let mut held = Vec::new();
        for _ in 0..3 {
            let (action, reply) = pending.recv().await.unwrap();
            if let Action::Request {
                command: Command::Submit { submission, .. },
                ..
            } = &action
            {
                submissions.insert(submission.task.payload.clone());
            }
            held.push((action, reply));
        }
        assert_eq!(submissions, BTreeSet::from(["slow".into(), "fast".into()]));
        drop(input); // EOF 不丢弃在途请求.
        assert!(!running.is_finished());
        for (action, reply) in held.into_iter().rev() {
            let outcome = match &action {
                Action::Request {
                    command: Command::Workers { .. },
                    ..
                } => Outcome::Workers { workers: workers() },
                Action::Request {
                    command: Command::Submit { submission, .. },
                    ..
                } => completed(&submission.task.payload),
                _ => panic!("unexpected discovery"),
            };
            reply
                .send(response(action, Ok(outcome)))
                .unwrap_or_else(|_| panic!("request receiver closed"));
        }
        let session = running.await.unwrap();
        assert_eq!(session.choice.worker, Some(Selection::Exact(instance(3))));
        assert!(
            pending.recv().await.is_none(),
            "requests must not be retried"
        );
    })
    .await;
}

/// 验证 Console 满载时仍可处理退出, 且不发送超额请求.
///
/// # 返回值
///
/// `()`; 全部断言通过时正常结束, 断言失败或未预期错误会使测试失败.
#[tokio::test]
async fn full_console_still_accepts_quit_and_does_not_send_excess_requests() {
    for limit in [1, 3, 32] {
        within(async {
            let mut session = selected();
            session.max_in_flight_requests = limit;
            let (input, receive) = mpsc::channel(limit + 4);
            let (events, mut pending) = mpsc::unbounded_channel();
            let running = tokio::spawn(async move {
                drive(&mut session, receive, move |action: Action| {
                    let events = events.clone();
                    async move {
                        let (send, receive) = oneshot::channel();
                        events
                            .send((action, send))
                            .unwrap_or_else(|_| panic!("test observer closed"));
                        receive.await.unwrap()
                    }
                })
                .await
            });
            let (discovery, reply) = pending.recv().await.unwrap();
            // 发现占一个名额; 其余名额全部占住, 不靠定时等待猜测任务是否已完成.
            let mut held = vec![(discovery, reply)];
            for i in 1..limit {
                input.send(Ok(format!("text-{i}"))).await.unwrap();
                held.push(pending.recv().await.unwrap());
            }
            input.send(Ok("overflow".into())).await.unwrap();
            input.send(Ok("/help".into())).await.unwrap();
            input.send(Ok("/quit".into())).await.unwrap();
            input.closed().await;
            assert!(!running.is_finished(), "quit must drain pending requests");
            assert!(
                pending.try_recv().is_err(),
                "full console sent an excess request"
            );
            for (action, reply) in held.into_iter().rev() {
                let completion = match action {
                    Action::Discover { request_id, .. } => Completion::Discovered {
                        request_id,
                        result: Ok(authorities()),
                    },
                    action => response(action, Err(anyhow::anyhow!("simulated request timeout"))),
                };
                reply
                    .send(completion)
                    .unwrap_or_else(|_| panic!("request receiver closed"));
            }
            running.await.unwrap().unwrap();
            assert!(
                pending.recv().await.is_none(),
                "failed requests must not be retried"
            );
        })
        .await;
    }
}

/// 验证中断 Console 会释放尚未完成的请求 future.
///
/// # 返回值
///
/// `()`; 全部断言通过时正常结束, 断言失败或未预期错误会使测试失败.
#[tokio::test]
async fn interrupting_console_drops_pending_futures() {
    /// 记录请求 future 是否被丢弃的测试通知 guard.
    struct DropNotice(
        /// 可取出并发送一次的资源释放通知.
        Option<oneshot::Sender<()>>,
    );
    impl Drop for DropNotice {
        /// 在测试 guard 被丢弃时记录 future 已释放.
        ///
        /// # 参数
        ///
        /// - `self`: 当前 `DropNotice` 的可变引用.
        ///
        /// # 返回值
        ///
        /// `()`; 更新测试观察状态.
        fn drop(&mut self) {
            if let Some(send) = self.0.take() {
                let _ = send.send(());
            }
        }
    }
    within(async {
        let mut session = selected();
        let (input, receive) = mpsc::channel(1);
        let (events, mut started) = mpsc::unbounded_channel();
        let running = tokio::spawn(async move {
            drive(&mut session, receive, move |_| {
                let events = events.clone();
                async move {
                    let (send, receive) = oneshot::channel();
                    let _drop = DropNotice(Some(send));
                    events.send(receive).unwrap();
                    std::future::pending().await
                }
            })
            .await
        });
        let discovery = started.recv().await.unwrap();
        input.send(Ok("pending".into())).await.unwrap();
        let submission = started.recv().await.unwrap();
        running.abort();
        assert!(running.await.unwrap_err().is_cancelled());
        discovery.await.unwrap();
        submission.await.unwrap();
        assert!(started.recv().await.is_none());
    })
    .await;
}
