use beamlet_kernel::{Closed, Operation};

#[derive(Debug)]
struct Identity;

#[derive(Debug)]
struct Request(String);

/// 验证关闭保持幂等且不额外要求身份类型 trait。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
#[test]
fn closes_without_identity_trait_bounds() {
    let operation = Operation::new(Identity, Request(String::from("request")));
    let closed: Operation<_, _, Closed> = operation.close();
    let closed = closed.close();

    assert_eq!(closed.request().0, "request");
}
