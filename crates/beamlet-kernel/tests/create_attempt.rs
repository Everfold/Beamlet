use beamlet_kernel::Operation;

#[derive(Debug, Clone)]
struct Identity(&'static str);

#[derive(Debug)]
struct Request(String);

/// 验证创建 Attempt 保持授权身份、Operation 身份和原始请求关联。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
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
