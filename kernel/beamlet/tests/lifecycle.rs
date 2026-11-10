use beamlet::lifecycle::{Completion, Error, Failure, Group, StopPolicy};
use std::{
    future::pending,
    io,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{
    runtime::Handle,
    sync::oneshot,
    time::{Instant, advance, sleep, timeout},
};

fn group<K>(seconds: u64) -> Group<K, io::Error> {
    Group::new(Handle::current(), Duration::from_secs(seconds)).unwrap()
}

struct Dropped(Arc<AtomicUsize>);
impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn construction_and_start_use_the_explicit_runtime_without_an_entered_runtime() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let mut drivers =
        Group::<_, io::Error>::new(runtime.handle().clone(), Duration::from_secs(5)).unwrap();
    let (started, running) = oneshot::channel();
    drivers
        .start(
            "explicit runtime",
            StopPolicy::Drain,
            |mut shutdown| async move {
                started.send(()).unwrap();
                shutdown.requested().await;
                Ok(())
            },
        )
        .unwrap();
    runtime.block_on(async {
        running.await.unwrap();
        let summary = drivers.drain().await;
        assert!(!summary.timed_out);
        assert_eq!(summary.exits[0].label, "explicit runtime");
        assert_eq!(
            summary.exits[0].result.as_ref().unwrap(),
            &Completion::Drained
        );
    });
}

#[tokio::test(start_paused = true)]
async fn empty_group_waits_for_explicit_stop_and_rejects_start_without_calling_constructor() {
    let mut drivers = group::<()>(5);
    assert!(
        timeout(Duration::from_secs(1), drivers.wait())
            .await
            .is_err()
    );
    drivers.stop();
    drivers.wait().await;
    let mut constructed = false;
    assert_eq!(
        drivers.start((), StopPolicy::Drain, |_| {
            constructed = true;
            async { Ok(()) }
        }),
        Err(Error::AlreadyStopping)
    );
    assert!(!constructed);
    let summary = drivers.drain().await;
    assert!(!summary.timed_out);
    assert!(summary.exits.is_empty());
}

#[tokio::test]
async fn invalid_deadlines_are_rejected_before_any_task_can_start() {
    for budget in [
        Duration::ZERO,
        Duration::from_nanos(999_999),
        Duration::from_secs(61),
    ] {
        assert!(matches!(
            Group::<(), io::Error>::new(Handle::current(), budget),
            Err(Error::InvalidDrainTimeout)
        ));
    }
}

#[tokio::test(start_paused = true)]
async fn stop_drains_roles_but_aborts_and_reaps_auxiliary_work() {
    let dropped = Arc::new(AtomicUsize::new(0));
    let guard = Dropped(Arc::clone(&dropped));
    let mut drivers = group(5);
    let (started, running) = oneshot::channel();
    drivers
        .start("announcements", StopPolicy::Abort, |_| async move {
            let _guard = guard;
            started.send(()).unwrap();
            pending().await
        })
        .unwrap();
    drivers
        .start("roles", StopPolicy::Drain, |mut shutdown| async move {
            assert!(!shutdown.is_draining());
            shutdown.requested().await;
            assert!(shutdown.is_draining());
            shutdown.requested().await;
            sleep(Duration::from_secs(2)).await;
            Ok(())
        })
        .unwrap();
    running.await.unwrap();
    tokio::task::yield_now().await;
    let summary = drivers.drain().await;
    assert!(!summary.timed_out);
    assert_eq!(
        dropped.load(Ordering::SeqCst),
        1,
        "auxiliary task must be reaped before returning"
    );
    assert_eq!(summary.exits.len(), 2);
    for exit in summary.exits {
        assert_eq!(
            exit.result.unwrap(),
            if exit.label == "roles" {
                Completion::Drained
            } else {
                Completion::Aborted
            }
        );
    }
}

#[tokio::test(start_paused = true)]
async fn cancelled_wait_loses_no_failure_and_other_roles_still_drain() {
    let mut drivers = group(5);
    let (fail, trigger) = oneshot::channel();
    drivers
        .start("failure", StopPolicy::Drain, |_| async move {
            trigger.await.unwrap();
            Err(io::Error::other("original failure"))
        })
        .unwrap();
    drivers
        .start("healthy", StopPolicy::Drain, |mut shutdown| async move {
            shutdown.requested().await;
            sleep(Duration::from_secs(2)).await;
            Ok(())
        })
        .unwrap();
    assert!(
        timeout(Duration::from_secs(1), drivers.wait())
            .await
            .is_err()
    );
    fail.send(()).unwrap();
    drivers.wait().await;
    let summary = drivers.drain().await;
    assert!(!summary.timed_out);
    assert_eq!(summary.exits.len(), 2);
    for exit in summary.exits {
        match exit.label {
            "failure" => assert!(
                matches!(exit.result, Err(Failure::Driver(error)) if error.to_string() == "original failure")
            ),
            "healthy" => assert_eq!(exit.result.unwrap(), Completion::Drained),
            _ => panic!("unexpected driver label"),
        }
    }
}

#[tokio::test(start_paused = true)]
async fn early_success_is_unconfirmed_even_if_joined_after_a_stop_request() {
    let mut drivers = group(5);
    let (finished, done) = oneshot::channel();
    drivers
        .start("early", StopPolicy::Drain, |_| async move {
            finished.send(()).unwrap();
            Ok(())
        })
        .unwrap();
    done.await.unwrap();
    // The driver already returned while Running, but its result has not been collected.
    drivers.stop();
    let summary = drivers.drain().await;
    assert!(matches!(
        summary.exits[0].result,
        Err(Failure::UnexpectedExit)
    ));
}

#[tokio::test(start_paused = true)]
async fn unexpected_success_requests_stop_of_remaining_drivers() {
    let mut drivers = group(5);
    drivers
        .start("early", StopPolicy::Abort, |_| async { Ok(()) })
        .unwrap();
    drivers
        .start("other", StopPolicy::Drain, |mut shutdown| async move {
            shutdown.requested().await;
            Ok(())
        })
        .unwrap();
    drivers.wait().await;
    let summary = drivers.drain().await;
    assert!(matches!(
        summary.exits[0].result,
        Err(Failure::UnexpectedExit)
    ));
    assert_eq!(
        summary.exits[1].result.as_ref().unwrap(),
        &Completion::Drained
    );
}

#[tokio::test(start_paused = true)]
async fn panic_keeps_the_driver_label_and_drains_other_roles() {
    let mut drivers = group(5);
    drivers
        .start(
            vec!["authority", "local-worker"],
            StopPolicy::Drain,
            |_| async {
                panic!("simulated driver panic");
            },
        )
        .unwrap();
    drivers
        .start(
            vec!["independent-worker"],
            StopPolicy::Drain,
            |mut shutdown| async move {
                shutdown.requested().await;
                Ok(())
            },
        )
        .unwrap();
    drivers.wait().await;
    let summary = drivers.drain().await;
    assert_eq!(summary.exits[0].label, ["authority", "local-worker"]);
    assert!(matches!(&summary.exits[0].result, Err(Failure::Task(error)) if error.is_panic()));
    assert_eq!(
        summary.exits[1].result.as_ref().unwrap(),
        &Completion::Drained
    );
}

#[tokio::test(start_paused = true)]
async fn all_roles_share_one_deadline_and_repeated_stop_does_not_extend_it() {
    let dropped = Arc::new(AtomicUsize::new(0));
    let mut drivers = group(5);
    for (label, seconds) in [("fast", 4), ("slow", 8)] {
        let guard = Dropped(Arc::clone(&dropped));
        drivers
            .start(label, StopPolicy::Drain, |mut shutdown| async move {
                let _guard = guard;
                shutdown.requested().await;
                sleep(Duration::from_secs(seconds)).await;
                Ok(())
            })
            .unwrap();
    }
    let started = Instant::now();
    drivers.stop();
    tokio::task::yield_now().await;
    advance(Duration::from_secs(3)).await;
    drivers.stop();
    let summary = drivers.drain().await;
    assert!(summary.timed_out);
    assert_eq!(started.elapsed(), Duration::from_secs(5));
    assert_eq!(
        dropped.load(Ordering::SeqCst),
        2,
        "timed-out task must also be reaped"
    );
    assert_eq!(summary.exits.len(), 2);
    for exit in summary.exits {
        match exit.label {
            "fast" => assert_eq!(exit.result.unwrap(), Completion::Drained),
            "slow" => assert!(matches!(exit.result, Err(Failure::DeadlineExceeded))),
            _ => panic!("unexpected driver label"),
        }
    }
}

#[tokio::test(start_paused = true)]
async fn delayed_drain_does_not_start_a_new_budget() {
    let dropped = Arc::new(AtomicUsize::new(0));
    let guard = Dropped(Arc::clone(&dropped));
    let mut drivers = group(5);
    drivers
        .start((), StopPolicy::Drain, |_| async move {
            let _guard = guard;
            pending().await
        })
        .unwrap();
    drivers.stop();
    advance(Duration::from_secs(6)).await;
    let started = Instant::now();
    let summary = drivers.drain().await;
    assert_eq!(started.elapsed(), Duration::ZERO);
    assert!(summary.timed_out);
    assert_eq!(dropped.load(Ordering::SeqCst), 1);
    assert!(matches!(
        summary.exits[0].result,
        Err(Failure::DeadlineExceeded)
    ));
}

#[tokio::test(start_paused = true)]
async fn failure_during_drain_does_not_abort_a_healthy_role() {
    let mut drivers = group(5);
    drivers
        .start("failed", StopPolicy::Drain, |mut shutdown| async move {
            shutdown.requested().await;
            Err(io::Error::other("drain failed"))
        })
        .unwrap();
    drivers
        .start("healthy", StopPolicy::Drain, |mut shutdown| async move {
            shutdown.requested().await;
            sleep(Duration::from_secs(3)).await;
            Ok(())
        })
        .unwrap();
    let summary = drivers.drain().await;
    assert!(!summary.timed_out);
    assert!(matches!(summary.exits[0].result, Err(Failure::Driver(_))));
    assert_eq!(
        summary.exits[1].result.as_ref().unwrap(),
        &Completion::Drained
    );
}

#[tokio::test]
async fn dropping_group_closes_stop_channel_and_requests_task_cancellation() {
    let dropped = Arc::new(AtomicUsize::new(0));
    let guard = Dropped(Arc::clone(&dropped));
    let mut drivers = group(5);
    let mut observer = None;
    drivers
        .start((), StopPolicy::Drain, |shutdown| {
            observer = Some(shutdown);
            async move {
                let _guard = guard;
                pending().await
            }
        })
        .unwrap();
    drop(drivers);
    let mut observer = observer.unwrap();
    observer.requested().await;
    assert!(observer.is_draining());
    timeout(Duration::from_secs(1), async {
        while dropped.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}
