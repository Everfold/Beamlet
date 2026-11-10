use beamlet::{
    composition::{Authority, Config, Error, Worker},
    lifecycle::DEFAULT_DRAIN_TIMEOUT,
    worker_observation::{Observations, Source},
};
use std::time::Duration;

fn authority(instance: u128, workers: &[u128]) -> Authority {
    Authority {
        id: (instance + 100).into(),
        instance: instance.into(),
        workers: workers.iter().copied().map(Into::into).collect(),
    }
}
fn worker(instance: u128, authority: u128) -> Worker {
    Worker {
        id: (instance + 100).into(),
        instance: instance.into(),
        authority: authority.into(),
    }
}
fn config() -> Config {
    Config {
        node_id: 1.into(),
        drain_timeout: DEFAULT_DRAIN_TIMEOUT,
        authorities: vec![authority(2, &[3])],
        workers: vec![worker(3, 2)],
    }
}

#[test]
fn role_plans_need_neither_a_runtime_nor_infrastructure_and_preserve_exact_input_order() {
    let mut input = config();
    input.authorities = vec![authority(20, &[32, 31, 30]), authority(2, &[3])];
    input.workers = vec![worker(30, 20), worker(3, 2), worker(40, 50), worker(32, 20)];
    let plan = input.clone().into_plan().unwrap();
    assert_eq!(plan.node_id(), input.node_id);
    assert_eq!(plan.drain_timeout(), input.drain_timeout);
    assert_eq!(plan.authorities(), input.authorities);
    assert_eq!(plan.workers(), input.workers);
    assert_eq!(
        plan.local_workers(20.into()).copied().collect::<Vec<_>>(),
        vec![worker(30, 20), worker(32, 20)]
    );
    assert_eq!(
        plan.local_workers(2.into()).copied().collect::<Vec<_>>(),
        vec![worker(3, 2)]
    );
    assert_eq!(
        plan.remote_workers().copied().collect::<Vec<_>>(),
        vec![worker(40, 50)]
    );
    assert_eq!(
        plan.local_workers(50.into()).count(),
        0,
        "remote Authority must not become a local combination"
    );
    let slots = plan.worker_slots(20.into()).collect::<Vec<_>>();
    assert_eq!(
        slots
            .iter()
            .map(|slot| (slot.instance.as_u128(), slot.source))
            .collect::<Vec<_>>(),
        vec![
            (32, Source::Local),
            (31, Source::Remote),
            (30, Source::Local)
        ]
    );
    assert!(Observations::new(20.into(), slots).is_ok());
    assert_eq!(plan.worker_slots(50.into()).count(), 0);
    let assignments = plan
        .authorities()
        .iter()
        .map(|a| plan.local_workers(a.instance).count())
        .sum::<usize>()
        + plan.remote_workers().count();
    assert_eq!(
        assignments,
        plan.workers().len(),
        "each Worker belongs to exactly one driver group"
    );
}

#[test]
fn empty_authority_only_and_worker_only_nodes_are_valid_explicit_choices() {
    let mut empty = config();
    empty.authorities.clear();
    empty.workers.clear();
    let plan = empty.into_plan().unwrap();
    assert!(plan.authorities().is_empty());
    assert!(plan.workers().is_empty());
    let mut authority_only = config();
    authority_only.workers.clear();
    let plan = authority_only.into_plan().unwrap();
    assert_eq!(
        plan.worker_slots(2.into()).next().unwrap().source,
        Source::Remote
    );
    assert_eq!(plan.local_workers(2.into()).count(), 0);
    let mut worker_only = config();
    worker_only.authorities.clear();
    assert_eq!(worker_only.into_plan().unwrap().remote_workers().count(), 1);
}

#[test]
fn time_and_role_limits_reject_invalid_configs_without_clamping_or_dropping_roles() {
    for duration in [
        Duration::ZERO,
        Duration::from_nanos(999_999),
        Duration::from_secs(60) + Duration::from_nanos(1),
    ] {
        let mut input = config();
        input.drain_timeout = duration;
        assert_eq!(input.into_plan(), Err(Error::InvalidDrainTimeout));
    }
    for duration in [Duration::from_millis(1), Duration::from_secs(60)] {
        let mut input = config();
        input.drain_timeout = duration;
        assert_eq!(input.into_plan().unwrap().drain_timeout(), duration);
    }
    let mut input = config();
    input.authorities.clear();
    input.workers = (1..=64).map(|id| worker(id, 1000)).collect();
    assert_eq!(input.clone().into_plan().unwrap().workers().len(), 64);
    input.workers.push(worker(65, 1000));
    assert_eq!(input.into_plan(), Err(Error::TooManyRoles));
    let mut input = config();
    input.workers.clear();
    input.authorities[0].workers = (100..164).map(Into::into).collect();
    assert_eq!(
        input.clone().into_plan().unwrap().authorities()[0]
            .workers
            .len(),
        64
    );
    input.authorities[0].workers.push(164.into());
    assert_eq!(input.into_plan(), Err(Error::TooManyWorkers(2.into())));
    let mut input = config();
    input.authorities[0].workers.clear();
    assert_eq!(input.into_plan(), Err(Error::NoWorkers(2.into())));
}

#[test]
fn stable_and_running_identities_are_unique_across_both_role_kinds() {
    let mut input = config();
    input.workers[0].id = 102.into();
    assert_eq!(input.into_plan(), Err(Error::DuplicateIdentity(102)));
    let mut input = config();
    input.workers[0].instance = 2.into();
    assert_eq!(input.into_plan(), Err(Error::DuplicateInstance(2.into())));
    let mut input = config();
    input.authorities.push(authority(4, &[5]));
    input.authorities[1].id = 102.into();
    assert_eq!(input.into_plan(), Err(Error::DuplicateIdentity(102)));
    let mut input = config();
    input.workers.push(worker(4, 9));
    input.workers[1].instance = 3.into();
    assert_eq!(input.into_plan(), Err(Error::DuplicateInstance(3.into())));
    // Stable identity and instance identity remain separate namespaces.
    let mut input = config();
    input.authorities[0].id = 3.into();
    assert!(input.into_plan().is_ok());
}

#[test]
fn local_role_references_must_be_reciprocal_and_cannot_target_the_wrong_kind() {
    let mut input = config();
    input.authorities[0].workers.push(3.into());
    assert_eq!(
        input.into_plan(),
        Err(Error::DuplicateWorker {
            authority: 2.into(),
            worker: 3.into()
        })
    );
    let mut input = config();
    input.authorities[0].workers = vec![2.into()];
    assert_eq!(
        input.into_plan(),
        Err(Error::WorkerTargetsAuthority(2.into()))
    );
    let mut input = config();
    input.authorities.push(authority(4, &[9]));
    input.authorities[0].workers = vec![4.into()];
    assert_eq!(
        input.into_plan(),
        Err(Error::WorkerTargetsAuthority(4.into()))
    );
    let mut input = config();
    input.workers[0].authority = 9.into();
    assert_eq!(
        input.into_plan(),
        Err(Error::WorkerBoundElsewhere {
            authority: 2.into(),
            worker: 3.into()
        })
    );
    let mut input = config();
    input.authorities[0].workers = vec![9.into()];
    assert_eq!(
        input.into_plan(),
        Err(Error::WorkerNotListed {
            authority: 2.into(),
            worker: 3.into()
        })
    );
    for target in [3, 4] {
        let mut input = config();
        input.authorities.clear();
        input.workers = vec![worker(3, target), worker(4, 9)];
        assert_eq!(
            input.into_plan(),
            Err(Error::AuthorityTargetsWorker(target.into())),
            "a known Worker cannot be treated as an unknown remote Authority"
        );
    }
    let mut input = config();
    input.authorities.push(authority(4, &[3]));
    assert_eq!(
        input.into_plan(),
        Err(Error::WorkerBoundElsewhere {
            authority: 4.into(),
            worker: 3.into()
        })
    );
}

#[tokio::test]
async fn caller_can_construct_and_execute_a_local_pair_from_the_plan_without_network_config() {
    use beamlet::{
        Authority as Driver, LocalExecution, Worker as Executor,
        memory::Limits,
        model::{ExecutionResult, ImpactStatus, Message, Operation, SubmitReport},
    };
    let plan = config().into_plan().unwrap();
    let configured = &plan.authorities()[0];
    let authority = Driver::<u32, u32, String>::new(
        configured.instance.into(),
        configured.workers.iter().copied().map(Into::into).collect(),
        Limits::default(),
    )
    .unwrap();
    let worker = Executor::new(
        configured.instance.into(),
        plan.local_workers(configured.instance)
            .map(|worker| worker.instance.into())
            .collect(),
        Limits::default(),
    )
    .unwrap();
    let mut execution = LocalExecution::from_parts(authority, worker);
    execution
        .submit(
            Operation {
                operation_id: 10.into(),
                task: 21,
            },
            20.into(),
        )
        .unwrap();
    let delivery = execution
        .next_report(|attempt| async move {
            SubmitReport {
                report_id: 30.into(),
                operation_id: attempt.operation.operation_id,
                attempt_id: attempt.attempt_id,
                impact: ImpactStatus::None,
                result: ExecutionResult::Succeeded {
                    output: attempt.operation.task * 2,
                },
            }
        })
        .await
        .unwrap()
        .unwrap();
    assert_eq!(delivery.source_instance_id, 3.into());
    assert_eq!(delivery.target_instance_id, 2.into());
    assert!(
        matches!(delivery.message,Message::SubmitReport(ref evidence) if evidence.result==ExecutionResult::Succeeded{output:42})
    );
    assert_eq!(execution.report(30.into()), Some(&delivery));
}
