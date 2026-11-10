use beamlet::{role_observation as roles, worker_observation as workers};
use std::{num::NonZeroU64, time::Duration};
use tokio::time::advance;

fn announcement(signal: workers::Signal) -> workers::Announcement {
    workers::Announcement {
        binding: workers::Binding {
            node_id: 1.into(),
            node_instance_id: 2.into(),
            worker_id: 3.into(),
            worker_instance_id: 4.into(),
            authority_instance_id: 5.into(),
        },
        signal,
    }
}
fn heartbeat(sequence: u64) -> workers::Announcement {
    announcement(workers::Signal::Heartbeat(
        NonZeroU64::new(sequence).unwrap(),
    ))
}
fn workers(source: workers::Source) -> workers::Observations {
    workers::Observations::new(
        5.into(),
        [workers::Slot {
            instance: 4.into(),
            source,
        }],
    )
    .unwrap()
}

#[tokio::test(start_paused = true)]
async fn startup_and_replay_do_not_refresh_or_restore_heartbeat_observation() {
    use workers::{Signal, Source, Status, Transition};
    let mut state = workers(Source::Remote);
    assert_eq!(state.snapshot(4.into()).unwrap().status, Status::Unobserved);
    advance(Duration::from_secs(3)).await;
    assert!(
        state
            .record(announcement(Signal::Started), Source::Remote)
            .unwrap()
            .binding_added
    );
    advance(Duration::from_secs(1)).await;
    assert_eq!(
        state.poll_expired(),
        Some(4.into()),
        "startup must not extend the original deadline"
    );
    assert!(state.poll_expired().is_none());
    assert_eq!(
        state
            .record(announcement(Signal::Started), Source::Remote)
            .unwrap()
            .transition,
        None
    );
    assert_eq!(state.snapshot(4.into()).unwrap().status, Status::Suspected);
    assert_eq!(
        state
            .record(heartbeat(7), Source::Remote)
            .unwrap()
            .transition,
        Some(Transition::Online)
    );
    advance(Duration::from_millis(3999)).await;
    for seq in [7, 6] {
        assert_eq!(
            state.record(heartbeat(seq), Source::Remote),
            Err(workers::Error::StaleHeartbeat)
        );
    }
    assert!(state.poll_expired().is_none());
    advance(Duration::from_millis(1)).await;
    assert_eq!(state.poll_expired(), Some(4.into()));
    assert_eq!(
        state.record(heartbeat(7), Source::Remote),
        Err(workers::Error::StaleHeartbeat)
    );
    assert_eq!(state.snapshot(4.into()).unwrap().status, Status::Suspected);
    assert_eq!(
        state
            .record(heartbeat(8), Source::Remote)
            .unwrap()
            .transition,
        Some(Transition::Online)
    );
}

#[tokio::test(start_paused = true)]
async fn heartbeat_can_bind_first_and_report_an_unpolled_expiration_before_recovery() {
    use workers::{Source, Transition};
    let mut state = workers(Source::Remote);
    let change = state.record(heartbeat(1), Source::Remote).unwrap();
    assert!(change.binding_added);
    assert_eq!(change.transition, Some(Transition::Online));
    advance(Duration::from_secs(1)).await;
    assert_eq!(
        state
            .record(heartbeat(2), Source::Remote)
            .unwrap()
            .transition,
        None
    );
    advance(Duration::from_secs(4)).await;
    assert_eq!(
        state
            .record(heartbeat(u64::MAX), Source::Remote)
            .unwrap()
            .transition,
        Some(Transition::ExpiredThenOnline)
    );
    assert!(state.poll_expired().is_none());
    assert_eq!(
        state.record(heartbeat(1), Source::Remote),
        Err(workers::Error::StaleHeartbeat)
    );
    assert_eq!(
        state.snapshot(4.into()).unwrap().sequence.unwrap().get(),
        u64::MAX
    );
}

#[tokio::test(start_paused = true)]
async fn invalid_binding_and_source_leave_the_snapshot_and_deadline_untouched() {
    use workers::Source;
    let mut state = workers(Source::Remote);
    assert_eq!(
        state.record(heartbeat(1), Source::Local),
        Err(workers::Error::WrongSource)
    );
    assert!(state.snapshot(4.into()).unwrap().binding.is_none());
    state.record(heartbeat(1), Source::Remote).unwrap();
    let original = state.snapshot(4.into()).unwrap();
    advance(Duration::from_secs(3)).await;
    for field in 0..5 {
        let mut changed = heartbeat(2);
        match field {
            0 => changed.binding.node_id = 99.into(),
            1 => changed.binding.node_instance_id = 99.into(),
            2 => changed.binding.worker_id = 99.into(),
            3 => changed.binding.worker_instance_id = 99.into(),
            _ => changed.binding.authority_instance_id = 99.into(),
        }
        assert!(state.record(changed, Source::Remote).is_err());
        assert_eq!(state.snapshot(4.into()), Some(original));
    }
    assert!(state.snapshot(99.into()).is_none());
    advance(Duration::from_secs(1)).await;
    assert_eq!(state.poll_expired(), Some(4.into()));
    let mut local = workers(Source::Local);
    assert_eq!(
        local.record(heartbeat(1), Source::Remote),
        Err(workers::Error::WrongSource)
    );
    assert!(local.snapshot(4.into()).unwrap().binding.is_none());
    local.record(heartbeat(1), Source::Local).unwrap();
}

#[tokio::test(start_paused = true)]
async fn slots_are_bounded_and_expire_independently_without_replacing_instances() {
    use workers::{Observations, Slot, Source, Status};
    assert!(
        Observations::new(5.into(), [])
            .unwrap()
            .poll_expired()
            .is_none()
    );
    let slot = Slot {
        instance: 4.into(),
        source: Source::Remote,
    };
    assert!(matches!(
        Observations::new(5.into(), [slot, slot]),
        Err(workers::Error::DuplicateInstance)
    ));
    assert!(
        Observations::new(
            5.into(),
            (0..64).map(|i| Slot {
                instance: i.into(),
                ..slot
            })
        )
        .is_ok()
    );
    assert!(matches!(
        Observations::new(
            5.into(),
            (0..65).map(|i| Slot {
                instance: i.into(),
                ..slot
            })
        ),
        Err(workers::Error::Capacity)
    ));
    let mut state = Observations::new(
        5.into(),
        [
            slot,
            Slot {
                instance: 6.into(),
                ..slot
            },
        ],
    )
    .unwrap();
    advance(Duration::from_secs(3)).await;
    state.record(heartbeat(1), Source::Remote).unwrap();
    advance(Duration::from_secs(1)).await;
    assert_eq!(state.poll_expired(), Some(6.into()));
    assert!(state.poll_expired().is_none());
    assert_eq!(state.snapshot(4.into()).unwrap().status, Status::Online);
    advance(Duration::from_secs(3)).await;
    assert_eq!(state.poll_expired(), Some(4.into()));
    assert!(state.poll_expired().is_none());
}

fn notice(instance: u128, phase: roles::Phase) -> roles::Notice {
    roles::Notice {
        binding: roles::Binding {
            node_id: 1.into(),
            node_instance_id: 2.into(),
            identity: roles::Identity::Worker(3.into()),
            instance: instance.into(),
        },
        phase,
    }
}

#[test]
fn exits_are_terminal_for_every_delivery_order_and_repeated_notice() {
    use roles::{ExitOutcome, Observations, Phase};
    for outcome in [ExitOutcome::Drained, ExitOutcome::Unconfirmed] {
        for exit_first in [false, true] {
            let mut state = Observations::new(1).unwrap();
            let started = notice(4, Phase::Started);
            let exited = notice(4, Phase::Exited(outcome));
            if !exit_first {
                assert!(state.record(started).unwrap());
            }
            assert!(state.record(exited).unwrap());
            assert!(!state.record(started).unwrap());
            assert!(!state.record(exited).unwrap());
            assert_eq!(state.snapshot(4.into()), Some(exited));
            let conflict = notice(
                4,
                Phase::Exited(match outcome {
                    ExitOutcome::Drained => ExitOutcome::Unconfirmed,
                    ExitOutcome::Unconfirmed => ExitOutcome::Drained,
                }),
            );
            assert_eq!(state.record(conflict), Err(roles::Error::OutcomeConflict));
            assert_eq!(state.snapshot(4.into()), Some(exited));
        }
    }
}

#[test]
fn role_identity_conflicts_are_atomic_and_new_instances_do_not_erase_history() {
    use roles::{Identity, Observations, Phase};
    let mut state = Observations::new(2).unwrap();
    let started = notice(4, Phase::Started);
    state.record(started).unwrap();
    for field in 0..4 {
        let mut changed = started;
        match field {
            0 => changed.binding.node_id = 99.into(),
            1 => changed.binding.node_instance_id = 99.into(),
            2 => changed.binding.identity = Identity::Worker(99.into()),
            _ => changed.binding.identity = Identity::Authority(3.into()),
        }
        assert_eq!(state.record(changed), Err(roles::Error::BindingConflict));
        assert_eq!(state.snapshot(4.into()), Some(started));
    }
    let new_instance = notice(5, Phase::Started);
    assert!(state.record(new_instance).unwrap());
    assert_eq!(state.snapshot(4.into()), Some(started));
    assert_eq!(state.snapshot(5.into()), Some(new_instance));
    assert_eq!(
        state.record(notice(6, Phase::Started)),
        Err(roles::Error::Capacity)
    );
    assert!(state.snapshot(6.into()).is_none());
    assert!(
        state
            .record(notice(4, Phase::Exited(roles::ExitOutcome::Drained)))
            .unwrap(),
        "existing record can advance at capacity"
    );
    assert!(!state.record(new_instance).unwrap());
    assert!(matches!(
        Observations::new(0),
        Err(roles::Error::InvalidLimit)
    ));
}
