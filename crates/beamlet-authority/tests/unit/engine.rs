use super::*;
use beamlet_primitives::{Attempt, Report};

#[test]
fn errors_preserve_pending_attempt() {
    let id = "operation".to_owned();
    let mut ledger = Ledger::new();
    ledger
        .create_operation(id.clone(), String::from("hello"))
        .unwrap();
    let attempt = ledger.create_attempt(&id, "attempt".into()).unwrap();

    for error in [
        DeliveryError::NoReceiver,
        DeliveryError::NotDelivered("full".into()),
        DeliveryError::Unknown("timeout".into()),
    ] {
        assert!(finish::<_, String>(&mut ledger, &id, Err(error)).is_err());
        assert_eq!(
            ledger
                .operation(&id)
                .unwrap()
                .pending_attempt_id()
                .map(String::as_str),
            Some("attempt")
        );
    }

    for (operation, attempt) in [("other", "attempt"), ("operation", "stale")] {
        let report: WorkerReport = Report::new(
            "bad".into(),
            Attempt::from_parts(operation.into(), attempt.into()),
            "bad".into(),
        );
        assert!(finish(&mut ledger, &id, Ok(report)).is_err());
        assert_eq!(
            ledger
                .operation(&id)
                .unwrap()
                .pending_attempt_id()
                .map(String::as_str),
            Some("attempt")
        );
    }

    let report: WorkerReport = Report::new("good".into(), attempt, "olleh".into());
    assert_eq!(finish(&mut ledger, &id, Ok(report)).unwrap(), "olleh");
    assert!(ledger.create_attempt(&id, "new".into()).is_err());

    let duplicate: WorkerReport = Report::new(
        "good".into(),
        Attempt::from_parts(id.clone(), "attempt".into()),
        "olleh".into(),
    );
    assert!(finish(&mut ledger, &id, Ok(duplicate)).is_err());
}
