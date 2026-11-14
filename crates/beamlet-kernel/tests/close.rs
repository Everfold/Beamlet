use beamlet_kernel::{Closed, Operation, Report};

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

/// 接受报告后可显式关闭, 重复关闭保留身份与请求.
#[test]
fn closes_after_report_and_is_idempotent() {
    let (operation, attempt) = Operation::new("operation-1", "request").create_attempt(1);
    let report = Report::new(1, attempt, "result");
    let operation = operation.accept_report(&report).expect("当前报告应被接受");
    let closed = operation.close().close().close();

    assert_eq!(closed.id(), &"operation-1");
    assert_eq!(closed.request(), &"request");
}
