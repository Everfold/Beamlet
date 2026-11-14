use beamlet_kernel::{AwaitingReport, Operation};

#[derive(Debug, Clone)]
struct Identity(&'static str);

#[derive(Debug)]
struct Request(String);

/// 创建时绑定身份并保留原始请求, 请求与身份均无需支持比较.
#[test]
fn creates_attempt_with_bound_identity_and_request() {
    let operation = Operation::new(Identity("operation-1"), Request(String::from("request")));
    let (operation, attempt) = operation.create_attempt(Identity("attempt-1"));

    assert_eq!(operation.id().0, "operation-1");
    assert_eq!(operation.request().0, "request");
    assert_eq!(operation.pending_attempt_id().0, "attempt-1");
    assert_eq!(attempt.operation_id().0, "operation-1");
    assert_eq!(attempt.id().0, "attempt-1");
}

/// 不同 Attempt 身份类型只影响等待状态, 新建 Operation 无需预先指定它.
#[test]
fn attempt_identity_is_chosen_when_creating() {
    let (first, _) = Operation::new(1, "same request").create_attempt("attempt-1");
    let (second, _) = Operation::new(2, "same request").create_attempt(42_u64);
    let first: Operation<_, _, AwaitingReport<&str>> = first;
    let second: Operation<_, _, AwaitingReport<u64>> = second;

    assert_eq!(first.pending_attempt_id(), &"attempt-1");
    assert_eq!(second.pending_attempt_id(), &42);
    assert_ne!(first.id(), second.id());
}
