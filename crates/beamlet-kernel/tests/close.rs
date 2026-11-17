use beamlet_kernel::{Closed, Operation};

/// 不附加比较或克隆约束的零大小身份, 用于验证关闭接口的最小约束.
#[derive(Debug)]
struct Identity;

/// 内核或台账测试的原始请求, 故意不实现 Clone, 以验证所有权保留.
#[derive(Debug)]
struct Request(
    /// 原始请求文本, 失败时必须原样归还或继续保留.
    String,
);

/// 验证关闭保持幂等且不额外要求身份类型 trait.
///
/// # 返回值
///
/// `()`; 全部断言通过时正常结束, 断言失败或未预期错误会使测试失败.
#[test]
fn closes_without_identity_trait_bounds() {
    let operation = Operation::new(Identity, Request(String::from("request")));
    let closed: Operation<_, _, Closed> = operation.close();
    let closed = closed.close();

    assert_eq!(closed.request().0, "request");
}
