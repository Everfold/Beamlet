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

/// Worker 并发测试的总等待期限: 8 秒, 防止执行控制错误导致挂起.
const DEADLINE: Duration = Duration::from_secs(8);

/// 给异步测试步骤施加整体等待期限, 防止回归时永久挂起.
async fn within<F: Future>(future: F) -> F::Output {
    timeout(DEADLINE, future)
        .await
        .expect("worker concurrency test timed out")
}
/// 按测试序号创建关联身份明确的 Worker 调用.
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
/// 包装真实本地 WorkerInbox, 并通知测试停止动作已发生.
struct ObservedInbox {
    /// 实际接收, 停止和排空调用的本地端点.
    inner: WorkerEndpoint,
    /// 停止输入后发送一次的测试观察通知.
    stopped: Option<oneshot::Sender<()>>,
}
impl WorkerInbox for ObservedInbox {
    /// 委托真实本地 Worker 入口接收调用.
    async fn receive(&mut self) -> Result<Option<Incoming>, TransportError> {
        self.inner.receive().await
    }
    /// 停止真实输入, 并通知测试观察者关闭已发生.
    async fn stop(&mut self) -> Result<(), TransportError> {
        self.inner.stop().await?;
        if let Some(stopped) = self.stopped.take() {
            let _ = stopped.send(());
        }
        Ok(())
    }
}
/// 执行名额计数 guard, 正常完成或取消时减少活动计数.
struct Active(
    /// 当前仍在执行的处理器数量.
    Arc<AtomicUsize>,
);
impl Drop for Active {
    /// 在测试执行结束或取消时释放活动任务计数.
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}
/// 测试驱动释放一个挂起处理器时指定的行为.
#[derive(Debug)]
enum Action {
    /// 正常执行倒序转换并产生业务结果.
    Execute,
    /// 返回可预期的执行错误, 不产生 Report.
    Reject,
    /// 触发处理器 panic, 用于验证角色监督接收异常.
    Panic,
}
/// 已经开始但尚未获准完成的处理器通知.
struct Started {
    /// 调用序号, 用于对应任务, 授权和结果.
    index: usize,
    /// 由测试指定本次处理器继续执行, 拒绝或 panic 的通道.
    release: oneshot::Sender<Action>,
}
/// Worker 并发测试的控制和观察集合, 拥有所有提交与运行句柄.
struct Harness {
    /// 向目标 Worker 交付调用的本地传输句柄.
    delivery: Local,
    /// 按序号保存的交付等待任务; 每项最终返回报告或错误.
    responses: BTreeMap<usize, JoinHandle<Result<WorkerReport, DeliveryError>>>,
    /// 收到每个处理器已经开始的通知.
    started: mpsc::UnboundedReceiver<Started>,
    /// 向运行中的 Worker 请求停止的一次性发送端.
    stop: oneshot::Sender<()>,
    /// 确认输入已关闭新接收的观察端.
    stopped: oneshot::Receiver<()>,
    /// Worker 主循环的异步任务句柄, 供测试等待或取消.
    running: JoinHandle<Result<(), RunError>>,
    /// 已观察到的最大同时活动处理器数.
    peak: Arc<AtomicUsize>,
}
/// 启动可控处理器的 Worker 并并发提交测试调用.
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
/// 校验测试报告与序号对应, 并提取报告身份.
fn check_report(report: WorkerReport, index: usize) -> String {
    assert_eq!(report.operation_id(), &index.to_string());
    assert_eq!(report.attempt_id(), &format!("attempt:{index}"));
    assert_eq!(
        report.result(),
        &format!("text:{index}").chars().rev().collect::<String>()
    );
    report.id().clone()
}

/// 验证有限 Worker 并发阻止超额准入, 停止后排空队列和运行中任务.
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

/// 验证 Worker 的零上限表示无限, 可以在完成前接收超过接收缓冲数量的任务.
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

/// 验证执行拒绝和调用者丢弃回复都会释放 Worker 并发名额.
#[tokio::test]
async fn execution_rejection_and_abandoned_reply_release_capacity() {
    within(async {
        let mut h = start(1, 3);
        let rejected = h.started.recv().await.unwrap();
        rejected.release.send(Action::Reject).unwrap();
        assert!(matches!(
            h.responses.remove(&rejected.index).unwrap().await.unwrap(),
            Err(DeliveryError::Unknown(reason)) if reason.contains("simulated capability rejection")
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

/// 验证业务处理器 panic 会传播到角色监督层.
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
