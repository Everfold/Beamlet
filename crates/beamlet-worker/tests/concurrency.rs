//! 以可控异步挂起验证真实 Worker 循环, 不靠文本大小或执行耗时猜测并发.
use beamlet_api::Uuid;
use beamlet_api::{
    Call, Delivery, DeliveryError, Incoming, Pool, RoleInstance, TransportError, WorkerInbox,
    WorkerReport,
};
use beamlet_primitives::Attempt;
use beamlet_transport_local::{Local, WorkerEndpoint};
use beamlet_worker::{
    Capabilities, Concurrency, Context, ExecutionError, Options, RunError, Worker,
};
use std::future::Future;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
    time::timeout,
};

const DEADLINE: Duration = Duration::from_secs(8);

/// 给异步测试步骤施加整体等待期限，防止回归时永久挂起。
///
/// # 参数
///
/// - `future`：需要在测试期限内完成的异步步骤。
///
/// # 返回值
///
/// future 的原始输出；超过测试期限会 panic 并使测试失败。
async fn within<F: Future>(future: F) -> F::Output {
    timeout(DEADLINE, future)
        .await
        .expect("worker concurrency test timed out")
}
/// 按测试序号创建关联身份明确的 Worker 调用。
///
/// # 参数
///
/// - `index`：测试调用序号，用于关联 Operation、Attempt 和业务数据。
///
/// # 返回值
///
/// 供指定测试 Worker 执行的 Call。
fn call(index: usize) -> Call {
    Call {
        pool: Pool::default(),
        authority: RoleInstance {
            role_id: Uuid::from_u128(10 + index as u128 % 2),
            session_id: Uuid::from_u128(12 + index as u128 % 2),
        },
        worker: RoleInstance {
            role_id: Uuid::from_u128(3),
            session_id: Uuid::from_u128(4),
        },
        attempt: Attempt::from_parts(index.to_string(), format!("attempt:{index}")),
        capability: "test.reverse/v1".into(),
        payload: format!("text:{index}"),
    }
}
struct ObservedInbox {
    inner: WorkerEndpoint,
    stopped: Option<oneshot::Sender<()>>,
}
impl WorkerInbox for ObservedInbox {
    /// 委托真实本地 Worker 入口接收调用。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `ObservedInbox` 的可变引用。
    ///
    /// # 返回值
    ///
    /// 底层 receive 的原始结果：调用、EOF 或传输错误。
    async fn receive(&mut self) -> Result<Option<Incoming>, TransportError> {
        self.inner.receive().await
    }
    /// 停止真实输入，并通知测试观察者关闭已发生。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `ObservedInbox` 的可变引用。
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`；底层输入关闭失败返回 TransportError。
    async fn stop(&mut self) -> Result<(), TransportError> {
        self.inner.stop().await?;
        if let Some(stopped) = self.stopped.take() {
            let _ = stopped.send(());
        }
        Ok(())
    }
}
struct Active(Arc<AtomicUsize>);
impl Drop for Active {
    /// 在测试执行结束或取消时释放活动任务计数。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Active` 的可变引用。
    ///
    /// # 返回值
    ///
    /// `()`；共享计数减一。
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}
#[derive(Debug)]
enum Action {
    Execute,
    Reject,
    Panic,
}
struct Started {
    index: usize,
    release: oneshot::Sender<Action>,
}
struct Harness {
    delivery: Local,
    responses: BTreeMap<usize, JoinHandle<Result<WorkerReport, DeliveryError>>>,
    started: mpsc::UnboundedReceiver<Started>,
    stop: oneshot::Sender<()>,
    stopped: oneshot::Receiver<()>,
    running: JoinHandle<Result<(), RunError>>,
    peak: Arc<AtomicUsize>,
}
/// 启动可控处理器的 Worker 并并发提交测试调用。
///
/// # 参数
///
/// - `limit`：Worker 并发上限；零表示不设角色层上限。
/// - `count`：要提交的测试调用总数，同时决定本地接收缓冲容量。
///
/// # 返回值
///
/// Harness，包含每项调用结果、释放执行入口、停止信号和并发峰值观察。
fn start(limit: usize, count: usize) -> Harness {
    let delivery = Local::new(count).unwrap();
    let (stop, signal) = oneshot::channel();
    let (stopped, observed) = oneshot::channel();
    let (send, started) = mpsc::unbounded_channel();
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let highest = peak.clone();
    let executor = move |payload: String, context: Context| {
        let active = active.clone();
        let highest = highest.clone();
        let send = send.clone();
        async move {
            let concurrent = active.fetch_add(1, Ordering::SeqCst) + 1;
            let _active = Active(active);
            highest.fetch_max(concurrent, Ordering::SeqCst);
            let (release, wait) = oneshot::channel();
            send.send(Started {
                index: context.operation_id().parse().unwrap(),
                release,
            })
            .map_err(|_| ExecutionError::new("test observer closed"))?;
            match wait.await.map_err(ExecutionError::new)? {
                Action::Execute => Ok(payload.chars().rev().collect()),
                Action::Reject => Err(ExecutionError::new("simulated capability rejection")),
                Action::Panic => panic!("simulated execution panic"),
            }
        }
    };
    let mut capabilities = Capabilities::new();
    capabilities.register("test.reverse/v1", executor).unwrap();
    let worker = Worker::builder(call(0).worker, capabilities)
        .options(Options {
            concurrency: if limit == 0 {
                Concurrency::Unlimited
            } else {
                Concurrency::Limited(limit)
            },
            ..Options::default()
        })
        .build()
        .unwrap();
    let (inner, presence) = delivery.bind_worker(worker.binding()).unwrap();
    let inbox = ObservedInbox {
        inner,
        stopped: Some(stopped),
    };
    let running = tokio::spawn(worker.run(inbox, presence, async {
        signal.await.map_err(std::io::Error::other)
    }));
    let responses = (0..count)
        .map(|index| {
            let delivery = delivery.clone();
            (
                index,
                tokio::spawn(async move { delivery.deliver(call(index), DEADLINE).await }),
            )
        })
        .collect();
    Harness {
        delivery,
        responses,
        started,
        stop,
        stopped: observed,
        running,
        peak,
    }
}
/// 校验测试报告与序号对应，并提取报告身份。
///
/// # 参数
///
/// - `report`：完整报告，包含 Operation/Attempt 关联、报告身份及业务结果。
/// - `index`：测试调用序号，用于关联 Operation、Attempt 和业务数据。
///
/// # 返回值
///
/// Report ID 的副本；Operation、Attempt 或结果不符时 panic。
fn check_report(report: WorkerReport, index: usize) -> String {
    assert_eq!(report.operation_id(), &index.to_string());
    assert_eq!(report.attempt_id(), &format!("attempt:{index}"));
    assert_eq!(
        report.result(),
        &format!("text:{index}").chars().rev().collect::<String>()
    );
    report.id().clone()
}

/// 验证有限 Worker 并发阻止超额准入，停止后排空队列和运行中任务。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
#[tokio::test]
async fn finite_limits_gate_admission_and_drain_queued_and_running_work() {
    within(async {
        for limit in [1, 2] {
            let total = 6;
            let mut h = start(limit, total);
            let mut blocked = Vec::new();
            for _ in 0..limit {
                blocked.push(h.started.recv().await.unwrap());
            }
            assert!(
                timeout(Duration::from_millis(40), h.started.recv())
                    .await
                    .is_err(),
                "limit {limit} admitted extra work"
            );
            let fast = blocked.pop().unwrap();
            fast.release.send(Action::Execute).unwrap();
            let report = h
                .responses
                .remove(&fast.index)
                .unwrap()
                .await
                .unwrap()
                .unwrap();
            let mut report_ids = BTreeSet::from([check_report(report, fast.index)]);
            if let Some(slow) = blocked.first() {
                assert!(
                    !h.responses[&slow.index].is_finished(),
                    "fast task waited for slow task"
                );
            }
            blocked.push(h.started.recv().await.unwrap()); // one completion immediately frees one slot
            h.stop.send(()).unwrap();
            h.stopped.await.unwrap(); // shutdown is observable even while every slot is occupied
            assert!(!h.running.is_finished(), "worker stopped before draining");
            assert_eq!(
                h.delivery.deliver(call(total), DEADLINE).await.unwrap_err(),
                DeliveryError::NoReceiver
            );
            for task in blocked {
                task.release.send(Action::Execute).unwrap();
            }
            for _ in limit + 1..total {
                h.started
                    .recv()
                    .await
                    .unwrap()
                    .release
                    .send(Action::Execute)
                    .unwrap();
            }
            for (index, response) in h.responses {
                assert!(report_ids.insert(check_report(response.await.unwrap().unwrap(), index)));
            }
            h.running.await.unwrap().unwrap();
            assert_eq!(h.peak.load(Ordering::SeqCst), limit);
            assert_eq!(report_ids.len(), total);
        }
    })
    .await;
}

/// 验证 Worker 的零上限表示无限，可以在完成前接收超过接收缓冲数量的任务。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
#[tokio::test]
async fn zero_admits_more_than_the_receive_capacity_without_waiting_for_completion() {
    within(async {
        let total = 21;
        let mut h = start(0, total);
        let mut blocked = Vec::new();
        for _ in 0..total {
            blocked.push(h.started.recv().await.unwrap());
        }
        assert_eq!(h.peak.load(Ordering::SeqCst), total);
        h.stop.send(()).unwrap();
        h.stopped.await.unwrap();
        assert!(!h.running.is_finished());
        for task in blocked.into_iter().rev() {
            task.release.send(Action::Execute).unwrap();
        }
        let mut report_ids = BTreeSet::new();
        for (index, response) in h.responses {
            assert!(report_ids.insert(check_report(response.await.unwrap().unwrap(), index)));
        }
        h.running.await.unwrap().unwrap();
        assert_eq!(report_ids.len(), total);
    })
    .await;
}

/// 验证执行拒绝和调用者丢弃回复都会释放 Worker 并发名额。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
#[tokio::test]
async fn execution_rejection_and_abandoned_reply_release_capacity() {
    within(async {
        let mut h = start(1, 3);
        let rejected = h.started.recv().await.unwrap();
        rejected.release.send(Action::Reject).unwrap();
        assert!(matches!(
            h.responses.remove(&rejected.index).unwrap().await.unwrap(),
            Err(DeliveryError::Unknown(_))
        ));
        let abandoned = h.started.recv().await.unwrap();
        let response = h.responses.remove(&abandoned.index).unwrap();
        response.abort();
        assert!(response.await.unwrap_err().is_cancelled());
        abandoned.release.send(Action::Execute).unwrap();
        let last = h.started.recv().await.unwrap();
        h.stop.send(()).unwrap();
        h.stopped.await.unwrap();
        last.release.send(Action::Execute).unwrap();
        check_report(
            h.responses
                .remove(&last.index)
                .unwrap()
                .await
                .unwrap()
                .unwrap(),
            last.index,
        );
        h.running.await.unwrap().unwrap();
        assert_eq!(h.peak.load(Ordering::SeqCst), 1);
    })
    .await;
}

/// 验证业务处理器 panic 会传播到角色监督层。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
#[tokio::test]
async fn execution_panic_is_propagated_to_role_supervision() {
    within(async {
        let mut h = start(2, 1);
        h.started
            .recv()
            .await
            .unwrap()
            .release
            .send(Action::Panic)
            .unwrap();
        let error = h.running.await.unwrap().unwrap_err();
        assert!(matches!(error, RunError::Task(_)));
        for response in h.responses.into_values() {
            assert!(response.await.unwrap().is_err());
        }
    })
    .await;
}
