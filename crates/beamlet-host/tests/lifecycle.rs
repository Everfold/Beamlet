use beamlet_host::{Failure, Host, Options};
use std::{
    future::pending,
    io,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::sync::oneshot;

/// 创建使用默认退出期限的测试 Host.
///
/// # 返回值
///
/// 新的空 Host; 默认参数构造失败时 panic.
fn host() -> Host {
    Host::new(Options::default()).unwrap()
}

/// 验证宿主构造和登记任务期间不轮询任务, 并校验退出期限.
///
/// # 返回值
///
/// `()`; 全部断言通过时正常结束, 断言失败或未预期错误会使测试失败.
#[test]
fn construction_validates_before_any_task_is_polled() {
    for timeout in [Duration::ZERO, Duration::MAX] {
        assert!(
            Host::new(Options {
                shutdown_timeout: timeout
            })
            .is_err()
        );
    }
    let mut host = host();
    host.add::<io::Error>("not started", async {
        panic!("add polled task without run")
    });
    drop(host);
}

/// 验证任一任务失败会停止同组任务并汇总名称和原始错误来源.
///
/// # 返回值
///
/// `()`; 全部断言通过时正常结束, 断言失败或未预期错误会使测试失败.
#[tokio::test]
async fn failure_stops_siblings_and_aggregates_errors_with_task_names_and_sources() {
    let mut host = host();
    let drained = Arc::new(AtomicUsize::new(0));
    for index in 0..3 {
        let shutdown = host.shutdown();
        let drained = drained.clone();
        host.add(format!("sibling-{index}"), async move {
            shutdown.wait().await.unwrap();
            drained.fetch_add(1, Ordering::SeqCst);
            if index == 0 {
                Ok(())
            } else {
                Err(io::Error::other(format!("drain-{index}")))
            }
        });
    }
    host.add("failed-role", async {
        Err(io::Error::other("original failure"))
    });
    let error = host.run(pending::<io::Result<()>>()).await.unwrap_err();
    assert_eq!(drained.load(Ordering::SeqCst), 3);
    assert_eq!(error.failures().len(), 3);
    assert!(error.unfinished().is_empty());
    let Failure::Task { name, source } = &error.failures()[0] else {
        panic!("expected task failure")
    };
    assert_eq!(name, "failed-role");
    assert_eq!(
        source.downcast_ref::<io::Error>().unwrap().to_string(),
        "original failure"
    );
    for text in ["original failure", "drain-1", "drain-2"] {
        assert!(error.to_string().contains(text));
    }
    assert!(std::error::Error::source(&error).is_some());
}

/// 验证后台 EOF 不停止长期服务, 晚等待者也能观察停止通知.
///
/// # 返回值
///
/// `()`; 全部断言通过时正常结束, 断言失败或未预期错误会使测试失败.
#[tokio::test]
async fn background_eof_keeps_services_running_and_stop_reaches_late_waiters() {
    let mut host = host();
    let shutdown = host.shutdown();
    let late = shutdown.clone();
    let (stopped, receive) = oneshot::channel();
    host.add("role", async move {
        shutdown.wait().await?;
        stopped.send(()).unwrap();
        Ok::<_, std::convert::Infallible>(())
    });
    let (signal, wait) = oneshot::channel();
    let (eof, finished) = oneshot::channel();
    let running = tokio::spawn(host.run_with_background(wait, async move {
        eof.send(()).unwrap();
        Ok::<_, io::Error>(())
    }));
    finished.await.unwrap();
    assert!(!running.is_finished(), "background EOF stopped services");
    signal.send(()).unwrap();
    running.await.unwrap().unwrap();
    receive.await.unwrap();
    late.wait().await.unwrap();
}

/// 验证空宿主继续等待外部信号并保留信号失败原因.
///
/// # 返回值
///
/// `()`; 全部断言通过时正常结束, 断言失败或未预期错误会使测试失败.
#[tokio::test]
async fn empty_host_waits_for_signal_and_preserves_signal_failure() {
    let (signal, wait) = oneshot::channel::<()>();
    let running = tokio::spawn(host().run(wait));
    tokio::task::yield_now().await;
    assert!(!running.is_finished());
    drop(signal);
    let error = running.await.unwrap().unwrap_err();
    assert!(
        matches!(&error.failures()[0], Failure::Signal { source } if source.downcast_ref::<oneshot::error::RecvError>().is_some())
    );
}

/// 验证任务提前成功, panic 和后台错误都能触发组停止.
///
/// # 返回值
///
/// `()`; 全部断言通过时正常结束, 断言失败或未预期错误会使测试失败.
#[tokio::test]
async fn early_success_panic_and_background_error_all_stop_the_group() {
    for case in 0..3 {
        let mut host = host();
        let shutdown = host.shutdown();
        let (stopped, receive) = oneshot::channel();
        host.add("sibling", async move {
            shutdown.wait().await?;
            stopped.send(()).unwrap();
            Ok::<_, std::convert::Infallible>(())
        });
        if case == 0 {
            host.add("early", async { Ok::<_, io::Error>(()) });
        }
        if case == 1 {
            host.add::<io::Error>("panicked", async { panic!("simulated role panic") });
        }
        let error = host
            .run_with_background(pending::<io::Result<()>>(), async move {
                if case == 2 {
                    Err(io::Error::other("input failed"))
                } else {
                    pending().await
                }
            })
            .await
            .unwrap_err();
        receive.await.unwrap();
        match (case, &error.failures()[0]) {
            (0, Failure::UnexpectedExit { name }) => assert_eq!(name, "early"),
            (1, Failure::Task { name, source }) => {
                assert_eq!(name, "panicked");
                assert!(source.to_string().contains("simulated role panic"));
            }
            (2, Failure::Background { source }) => assert_eq!(source.to_string(), "input failed"),
            _ => panic!("unexpected failure: {error}"),
        }
    }
}

/// 测试资源清理 guard; Drop 唤醒等待清理完成的观察者.
struct Dropped(
    /// 资源被释放后发送通知的共享 Notify.
    Arc<tokio::sync::Notify>,
);
impl Drop for Dropped {
    /// 在测试资源被释放时发送一次 Drop 通知.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Dropped` 的可变引用.
    ///
    /// # 返回值
    ///
    /// `()`; 唤醒正在检查清理行为的观察者.
    fn drop(&mut self) {
        self.0.notify_one();
    }
}

/// 验证排空期限中止等待, 保留首次与后续错误且不延长期限.
///
/// # 返回值
///
/// `()`; 全部断言通过时正常结束, 断言失败或未预期错误会使测试失败.
#[tokio::test(start_paused = true)]
async fn deadline_aborts_pending_work_and_preserves_first_failure() {
    for failed in [false, true] {
        let mut host = Host::new(Options {
            shutdown_timeout: Duration::from_millis(20),
        })
        .unwrap();
        let dropped = Arc::new(tokio::sync::Notify::new());
        let guard = Dropped(dropped.clone());
        host.add("stuck", async move {
            let _guard = guard;
            pending::<io::Result<()>>().await
        });
        if failed {
            host.add("failed", async { Err(io::Error::other("original")) });
            let shutdown = host.shutdown();
            host.add("drain-error", async move {
                shutdown.wait().await.unwrap();
                tokio::time::sleep(Duration::from_millis(15)).await;
                Err(io::Error::other("later failure"))
            });
        }
        let before = tokio::time::Instant::now();
        let error = host
            .run(async move {
                if failed {
                    pending().await
                } else {
                    Ok::<_, io::Error>(())
                }
            })
            .await
            .unwrap_err();
        assert_eq!(
            tokio::time::Instant::now() - before,
            Duration::from_millis(20)
        );
        assert_eq!(error.unfinished(), &["stuck"]);
        assert_eq!(error.failures().len(), if failed { 2 } else { 0 });
        if failed {
            assert!(error.to_string().contains("original"));
        }
        assert!(error.to_string().contains("shutdown timed out"));
        dropped.notified().await;
    }
}

/// 验证丢弃运行 future 释放任务和后台输入, 并通知停止观察者.
///
/// # 返回值
///
/// `()`; 全部断言通过时正常结束, 断言失败或未预期错误会使测试失败.
#[tokio::test]
async fn dropping_run_cancels_owned_tasks_background_and_notifies_observers() {
    let mut host = host();
    let observer = host.shutdown();
    let task_dropped = Arc::new(tokio::sync::Notify::new());
    let background_dropped = Arc::new(tokio::sync::Notify::new());
    let guard = Dropped(task_dropped.clone());
    let (started, receive) = oneshot::channel();
    host.add("pending", async move {
        let _guard = guard;
        started.send(()).unwrap();
        pending::<io::Result<()>>().await
    });
    let guard = Dropped(background_dropped.clone());
    let running = tokio::spawn(
        host.run_with_background(pending::<io::Result<()>>(), async move {
            let _guard = guard;
            pending::<io::Result<()>>().await
        }),
    );
    receive.await.unwrap();
    running.abort();
    assert!(running.await.unwrap_err().is_cancelled());
    observer.wait().await.unwrap();
    task_dropped.notified().await;
    background_dropped.notified().await;
}

/// 验证停止一个 Host 不影响另一个 Host.
///
/// # 返回值
///
/// `()`; 全部断言通过时正常结束, 断言失败或未预期错误会使测试失败.
#[tokio::test]
async fn stopping_one_group_does_not_stop_another() {
    let first = host();
    let second = host();
    let second_shutdown = second.shutdown();
    let (stop_first, wait_first) = oneshot::channel();
    let (stop_second, wait_second) = oneshot::channel();
    let first = tokio::spawn(first.run(wait_first));
    let second = tokio::spawn(second.run(wait_second));
    stop_first.send(()).unwrap();
    first.await.unwrap().unwrap();
    assert!(!second.is_finished());
    assert!(
        tokio::time::timeout(Duration::from_millis(10), second_shutdown.clone().wait())
            .await
            .is_err()
    );
    stop_second.send(()).unwrap();
    second.await.unwrap().unwrap();
    second_shutdown.wait().await.unwrap();
}

/// 验证开始停止时先丢弃后台输入, 避免服务排空依赖它而卡住.
///
/// # 返回值
///
/// `()`; 全部断言通过时正常结束, 断言失败或未预期错误会使测试失败.
#[tokio::test]
async fn shutdown_drops_background_before_waiting_for_service_drain() {
    let mut host = host();
    let input_dropped = Arc::new(tokio::sync::Notify::new());
    let observed = input_dropped.clone();
    let shutdown = host.shutdown();
    host.add("waits for input release", async move {
        shutdown.wait().await?;
        observed.notified().await;
        Ok::<_, std::convert::Infallible>(())
    });
    let guard = Dropped(input_dropped);
    host.run_with_background(async { Ok::<_, io::Error>(()) }, async move {
        let _guard = guard;
        pending::<io::Result<()>>().await
    })
    .await
    .unwrap();
}
