use beamlet_kernel::Operation;

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
