//! 以可控执行阻塞验证真实 Worker 循环, 不靠文本大小或执行耗时猜测并发.
use super::*;
use crate::{
    delivery::{self, Delivery as _, DeliveryError, Incoming, channel},
    text_reverse,
};
use beamlet_primitives::Attempt;
use beamlet_protocol::{Pool, RoleInstance};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc as blocking,
    },
    time::Duration,
};
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
    time::timeout,
};
use uuid::Uuid;

const DEADLINE: Duration = Duration::from_secs(8);

async fn within<F: Future>(future: F) -> F::Output {
    timeout(DEADLINE, future)
        .await
        .expect("worker concurrency test timed out")
}
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
        capability: text_reverse::CAPABILITY.into(),
        text: format!("text:{index}"),
    }
}
struct ObservedInbox {
    inner: channel::Inbox,
    stopped: Option<oneshot::Sender<()>>,
}
impl Inbox for ObservedInbox {
    async fn receive(&mut self) -> Result<Option<Incoming>> {
        self.inner.receive().await
    }
    async fn stop(&mut self) -> Result<()> {
        self.inner.stop().await?;
        if let Some(stopped) = self.stopped.take() {
            let _ = stopped.send(());
        }
        Ok(())
    }
}
struct Active(Arc<AtomicUsize>);
impl Drop for Active {
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
    release: blocking::Sender<Action>,
}
struct Harness {
    delivery: channel::Delivery,
    responses: BTreeMap<usize, JoinHandle<Result<WorkerReport, DeliveryError>>>,
    started: mpsc::UnboundedReceiver<Started>,
    stop: oneshot::Sender<()>,
    stopped: oneshot::Receiver<()>,
    running: JoinHandle<Result<()>>,
    peak: Arc<AtomicUsize>,
}
fn start(limit: usize, count: usize) -> Harness {
    let mut delivery = channel::Delivery::new(DEADLINE);
    let inner = delivery.register(&[text_reverse::CAPABILITY], count);
    let (stop, signal) = oneshot::channel();
    let (stopped, observed) = oneshot::channel();
    let inbox = ObservedInbox {
        inner,
        stopped: Some(stopped),
    };
    let (send, started) = mpsc::unbounded_channel();
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let highest = peak.clone();
    let executor = move |call: Call, report_id| {
        let concurrent = active.fetch_add(1, Ordering::SeqCst) + 1;
        let _active = Active(active.clone());
        highest.fetch_max(concurrent, Ordering::SeqCst);
        let (release, wait) = blocking::channel();
        send.send(Started {
            index: call.attempt.operation_id().parse().unwrap(),
            release,
        })
        .map_err(|_| anyhow::anyhow!("test observer closed"))?;
        match wait
            .recv_timeout(DEADLINE)
            .context("test did not release execution")?
        {
            Action::Execute => execute(call, report_id),
            Action::Reject => anyhow::bail!("simulated capability rejection"),
            Action::Panic => panic!("simulated execution panic"),
        }
    };
    let running = tokio::spawn(run_with(
        inbox,
        "session".into(),
        limit,
        async { signal.await.map_err(std::io::Error::other) },
        executor,
    ));
    let responses = (0..count)
        .map(|index| {
            let delivery = delivery.clone();
            (
                index,
                tokio::spawn(async move { delivery.deliver(call(index)).await }),
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
fn check_report(report: WorkerReport, index: usize) -> String {
    assert_eq!(report.operation_id(), &index.to_string());
    assert_eq!(report.attempt_id(), &format!("attempt:{index}"));
    assert_eq!(
        report.result(),
        &format!("text:{index}").chars().rev().collect::<String>()
    );
    report.id().clone()
}

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
                h.delivery.deliver(call(total)).await.unwrap_err(),
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

#[tokio::test]
async fn zero_admits_more_than_the_receive_capacity_without_waiting_for_completion() {
    within(async {
        let total = delivery::RECEIVE_CAPACITY + 5;
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
        assert!(format!("{error:#}").contains("能力执行任务异常退出"));
        for response in h.responses.into_values() {
            assert!(response.await.unwrap().is_err());
        }
    })
    .await;
}
