use beamlet_kernel::Operation;

/// 自定义 Operation 身份, 用于验证授权创建中的身份关联.
#[derive(Debug, Clone)]
struct Identity(
    /// 该测试身份的静态名称.
    &'static str,
);

/// 内核或台账测试的原始请求, 故意不实现 Clone, 以验证所有权保留.
#[derive(Debug)]
struct Request(
    /// 原始请求文本, 失败时必须原样归还或继续保留.
    String,
);

/// 验证创建 Attempt 保持授权身份, Operation 身份和原始请求关联.
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
