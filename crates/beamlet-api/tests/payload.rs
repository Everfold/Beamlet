use beamlet_api::{
    Call, Outcome, Pool, Responder, RoleInstance, Selection, Submission, Task, Uuid, WorkerReport,
};
use beamlet_primitives::{Attempt, Report};
use tokio::sync::oneshot;

#[derive(Debug)]
struct Job(Vec<u32>);
#[derive(Debug, PartialEq, Eq)]
struct Total(u32);

#[tokio::test]
async fn custom_payload_and_result_need_neither_clone_nor_serialization() {
    let instance = RoleInstance {
        role_id: Uuid::from_u128(1),
        session_id: Uuid::from_u128(2),
    };
    let submission = Submission {
        pool: Pool::default(),
        worker: Selection::Exact(instance),
        task: Task {
            capability: "sum/v1".into(),
            payload: Job(vec![2, 3, 5]),
        },
    };
    let call = Call {
        pool: submission.pool,
        authority: instance,
        worker: instance,
        attempt: Attempt::from_parts("operation".into(), "attempt".into()),
        capability: submission.task.capability,
        payload: submission.task.payload,
    };
    let report: WorkerReport<Total> = Report::new(
        "report".into(),
        call.attempt,
        Total(call.payload.0.iter().sum()),
    );
    let (send, receive) = oneshot::channel();
    let reply = Responder::new(move |outcome| async move {
        send.send(outcome)
            .map_err(|_| beamlet_api::ReplyError::new(std::io::Error::other("closed")))
    });
    let (report_id, attempt, result) = report.into_parts();
    let (operation_id, attempt_id) = attempt.into_parts();
    reply
        .send(Outcome::Completed {
            operation_id,
            attempt_id,
            report_id,
            worker: instance,
            result,
        })
        .await
        .unwrap();
    let Outcome::Completed {
        result,
        operation_id,
        attempt_id,
        ..
    } = receive.await.unwrap()
    else {
        panic!("completed expected")
    };
    assert_eq!(result, Total(10));
    assert_eq!(
        (operation_id.as_str(), attempt_id.as_str()),
        ("operation", "attempt")
    );
}

#[test]
fn capability_validation_uses_bytes_and_preserves_the_exact_name() {
    use beamlet_api::CapabilityId;
    let name = " Text.能力/v1 ";
    let id: CapabilityId = name.parse().unwrap();
    assert_eq!(id.as_str(), name);
    assert!("".parse::<CapabilityId>().is_err());
    assert!("é".repeat(128).parse::<CapabilityId>().is_ok());
    assert!("é".repeat(129).parse::<CapabilityId>().is_err());
}
