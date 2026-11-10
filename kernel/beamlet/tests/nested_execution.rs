use beamlet::{
    Authority, Worker,
    memory::Limits,
    model::{DispatchAttempt, ExecutionResult, ImpactStatus, SubmitReport},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::sync::{Notify, Semaphore, mpsc};

#[derive(Clone, Debug, Eq, PartialEq)]
enum Task {
    Parent(usize),
    Child(usize),
}

#[tokio::test]
async fn waiting_parents_do_not_block_children_and_leaf_resource_limits_remain_explicit() {
    let limits = Limits {
        max_records: 32,
        queue_capacity: 16,
    };
    let mut authority = Authority::<Task, (), ()>::new(1.into(), vec![2.into()], limits).unwrap();
    let mut worker = Worker::new(1.into(), vec![2.into()], limits).unwrap();
    let (requests, mut children) = mpsc::channel(8);
    let gates = Arc::new((0..8).map(|_| Notify::new()).collect::<Vec<_>>());
    let resource = Arc::new(Semaphore::new(1));
    let active = Arc::new(AtomicUsize::new(0));
    let maximum = Arc::new(AtomicUsize::new(0));
    let execute = |attempt: DispatchAttempt<Task>| {
        let requests = requests.clone();
        let gates = Arc::clone(&gates);
        let resource = Arc::clone(&resource);
        let active = Arc::clone(&active);
        let maximum = Arc::clone(&maximum);
        async move {
            match attempt.operation.task {
                Task::Parent(index) => {
                    requests.send(index).await.unwrap();
                    gates[index].notified().await;
                }
                Task::Child(index) => {
                    {
                        let _permit = resource.acquire().await.unwrap();
                        let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                        maximum.fetch_max(current, Ordering::SeqCst);
                        tokio::task::yield_now().await;
                        active.fetch_sub(1, Ordering::SeqCst);
                    }
                    gates[index].notify_one();
                }
            }
            SubmitReport {
                report_id: attempt.attempt_id.as_u128().into(),
                operation_id: attempt.operation.operation_id,
                attempt_id: attempt.attempt_id,
                impact: ImpactStatus::None,
                result: ExecutionResult::Succeeded { output: () },
            }
        }
    };
    for index in 0..8 {
        let id = index as u128 + 1;
        let accepted = authority
            .submit_keyed(id.into(), Task::Parent(index), || (id.into(), id.into()))
            .unwrap();
        worker.receive_dispatch(accepted.dispatch).unwrap();
    }
    assert_eq!(worker.start_ready(execute), 8);
    tokio::time::timeout(Duration::from_secs(2), async {
        for _ in 0..8 {
            let index = children.recv().await.unwrap();
            let id = index as u128 + 9;
            let accepted = authority
                .submit_keyed(id.into(), Task::Child(index), || (id.into(), id.into()))
                .unwrap();
            worker.receive_dispatch(accepted.dispatch).unwrap();
        }
        // All eight original operations are still waiting on these children.
        assert_eq!(worker.start_ready(execute), 8);
        let mut completed = 0;
        while let Some(report) = worker.next_report(execute).await.unwrap() {
            authority.receive_report(report).unwrap();
            completed += 1;
        }
        assert_eq!(completed, 16);
    })
    .await
    .expect("children must progress while their parents are waiting");
    assert_eq!(maximum.load(Ordering::SeqCst), 1);
    assert_eq!(active.load(Ordering::SeqCst), 0);
    assert_eq!(worker.pending(), 0);
}
