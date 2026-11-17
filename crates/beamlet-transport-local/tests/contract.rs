#[path = "../../../tests/support/transport_contract.rs"]
mod contract;
use beamlet_transport_local::Local;

/// 用本地适配器验证共享的定向交付契约。
///
/// # 参数
///
/// 无。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
#[tokio::test]
async fn delivery_contract() {
    let transport = Local::new(8).unwrap();
    let (inbox, _presence) = transport.bind_worker(contract::binding()).unwrap();
    contract::delivery(transport, inbox).await;
}
/// 用本地适配器验证共享的发现、请求和公告控制契约。
///
/// # 参数
///
/// 无。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
#[tokio::test]
async fn control_contract() {
    let transport = Local::new(8).unwrap();
    let endpoint = transport.bind_authority(&contract::authority()).unwrap();
    let (_worker, presence) = transport.bind_worker(contract::binding()).unwrap();
    contract::control(transport, endpoint, presence).await;
}
