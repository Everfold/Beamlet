use beamlet_kernel::{Closed, Operation};

#[derive(Debug)]
struct Identity;

#[derive(Debug)]
struct Request(String);

/// 从未执行的 Operation 可以关闭, 身份和请求无需克隆或比较.
#[test]
fn closes_without_identity_trait_bounds() {
    let operation = Operation::new(Identity, Request(String::from("request")));
    let closed: Operation<_, _, Closed> = operation.close();
    let closed = closed.close();

    assert_eq!(closed.request().0, "request");
}
