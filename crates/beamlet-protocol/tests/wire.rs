use beamlet_api::{Pool, RoleInstance, Selection};
use beamlet_protocol::{self as protocol, Command, Submission, Task};
use uuid::Uuid;

#[test]
fn messages_preserve_identity_and_text_and_reject_unknown_versions() {
    let authority = RoleInstance {
        role_id: Uuid::new_v4(),
        session_id: Uuid::new_v4(),
    };
    let request = Command::Submit {
        request_id: Uuid::new_v4(),
        authority,
        submission: Submission {
            pool: Pool::default(),
            worker: Selection::Exact(authority),
            task: Task {
                capability: "text.uppercase/v1".into(),
                text: " ß\n世界 ".into(),
            },
        },
    };
    let encoded = protocol::encode(&request).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(
        value["body"]["authority"]["role_id"],
        authority.role_id.to_string()
    );
    assert!(value["body"]["authority"].get("node_id").is_none());
    let decoded: Command = protocol::decode(&encoded).unwrap();
    assert_eq!(decoded.request_id(), request.request_id());
    assert_eq!(decoded.authority(), authority);
    let Command::Submit { submission, .. } = decoded else {
        panic!("submit expected")
    };
    assert_eq!(submission.task.text, " ß\n世界 ");
    assert_eq!(submission.worker, Selection::Exact(authority));
    let bad = String::from_utf8(encoded)
        .unwrap()
        .replace("\"version\":3", "\"version\":99");
    assert!(matches!(
        protocol::decode::<Command>(bad.as_bytes()),
        Err(protocol::Error::Version(99))
    ));
}

#[test]
fn semantic_requests_keep_the_v3_text_profile() {
    let authority = RoleInstance {
        role_id: Uuid::from_u128(1),
        session_id: Uuid::from_u128(2),
    };
    let request_id = Uuid::from_u128(3);
    let semantic = beamlet_api::Command::Submit {
        request_id,
        authority,
        submission: beamlet_api::Submission {
            pool: Pool::default(),
            worker: Selection::Exact(authority),
            task: beamlet_api::Task {
                capability: "text.reverse/v1".into(),
                payload: " ß\n世界 ".into(),
            },
        },
    };
    let bytes = protocol::encode(&Command::from(semantic)).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        value,
        serde_json::json!({
            "version": 3,
            "body": {
                "type": "submit", "request_id": request_id, "authority": authority,
                "submission": { "pool": "default", "worker": { "exact": authority },
                    "task": { "capability": "text.reverse/v1", "text": " ß\n世界 " } }
            }
        })
    );
    let decoded: beamlet_api::Command = protocol::decode::<Command>(&bytes).unwrap().into();
    let beamlet_api::Command::Submit { submission, .. } = decoded else {
        panic!("submit expected")
    };
    assert_eq!(submission.task.payload, " ß\n世界 ");
}

#[test]
fn call_and_report_conversion_preserves_authorization_and_result() {
    use beamlet_primitives::{Attempt, Report};
    let instance = RoleInstance {
        role_id: Uuid::from_u128(1),
        session_id: Uuid::from_u128(2),
    };
    let call = beamlet_api::Call {
        pool: Pool::default(),
        authority: instance,
        worker: instance,
        attempt: Attempt::from_parts("operation".into(), "attempt".into()),
        capability: "text.reverse/v1".into(),
        payload: " 你好\n ".into(),
    };
    let bytes = protocol::encode(&beamlet_protocol::WorkerCall::from(call)).unwrap();
    let call: beamlet_api::Call = protocol::decode::<beamlet_protocol::WorkerCall>(&bytes)
        .unwrap()
        .into();
    assert_eq!(call.payload, " 你好\n ");
    assert_eq!(call.worker, instance);
    assert_eq!(call.attempt.id(), "attempt");
    let report: beamlet_api::WorkerReport =
        Report::new("report".into(), call.attempt, " 世界 ".into());
    let data = beamlet_protocol::ReportData::from(report);
    let bytes = protocol::encode(&data).unwrap();
    let roundtrip: beamlet_api::WorkerReport =
        protocol::decode::<beamlet_protocol::ReportData>(&bytes)
            .unwrap()
            .into();
    assert_eq!(roundtrip.operation_id(), "operation");
    assert_eq!(roundtrip.attempt_id(), "attempt");
    assert_eq!(roundtrip.id(), "report");
    assert_eq!(roundtrip.result(), " 世界 ");
}
