use beamlet_agent::{Agent, AgentError, Limits, StateError};

#[test]
fn legacy_names_still_construct_the_shared_local_execution_type() {
    let execution: beamlet::LocalExecution<(), (), ()> =
        Agent::new(1.into(), vec![2.into()], Limits::default()).unwrap();
    let (authority, worker) = execution.into_parts();
    let _legacy: Agent<(), (), ()> = beamlet::LocalExecution::from_parts(authority, worker);

    let invalid: Result<beamlet::LocalExecution<(), (), ()>, AgentError> =
        Agent::new(1.into(), vec![], Limits::default());
    assert!(matches!(
        invalid,
        Err(AgentError::State(StateError::InvalidWorkers))
    ));
}
