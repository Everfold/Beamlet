use beamlet_protocol::{
    self as protocol, Command, Pool, RoleInstance, Selection, Submission, Task, subject,
};
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
fn pools_cannot_escape_subject_tokens_and_sessions_route_separately() {
    for invalid in ["", "a.b", "a*", ">", "a b", " a", "\n", "池"] {
        assert!(invalid.parse::<Pool>().is_err(), "{invalid:?}");
    }
    let a: Pool = "A_1-test".parse().unwrap();
    let b: Pool = "a_1-test".parse().unwrap();
    assert_ne!(a, b);
    let instance = RoleInstance {
        role_id: Uuid::new_v4(),
        session_id: Uuid::new_v4(),
    };
    let restarted = RoleInstance {
        session_id: Uuid::new_v4(),
        ..instance
    };
    assert_ne!(
        subject::worker(&a, instance),
        subject::worker(&a, restarted)
    );
    assert_ne!(subject::worker(&a, instance), subject::worker(&b, instance));
    assert_ne!(
        subject::worker(&a, instance),
        subject::authority(&a, instance)
    );
}
