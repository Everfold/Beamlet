#[path = "../../../tests/support/transport_contract.rs"]
mod contract;
use beamlet_transport_local::Local;

#[tokio::test]
async fn delivery_contract() {
    let transport = Local::new(8).unwrap();
    let (inbox, _presence) = transport.bind_worker(contract::binding()).unwrap();
    contract::delivery(transport, inbox).await;
}
#[tokio::test]
async fn control_contract() {
    let transport = Local::new(8).unwrap();
    let endpoint = transport.bind_authority(&contract::authority()).unwrap();
    let (_worker, presence) = transport.bind_worker(contract::binding()).unwrap();
    contract::control(transport, endpoint, presence).await;
}
