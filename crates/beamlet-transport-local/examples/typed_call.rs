//! 仅使用公开传输接口传递自定义数据, 不读取配置或连接消息服务器.
use beamlet_api::{Call, Delivery, Pool, RoleInstance, Uuid, WorkerBinding, WorkerInbox};
use beamlet_primitives::{Attempt, Report};
use beamlet_transport_local::Local;
use std::{error::Error, time::Duration};

// 请求/结果无需 Clone、Debug 或 serde.
struct Sum(Vec<i64>);
struct Total(i64);

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let transport = Local::<Sum, Total>::new(8)?;
    let instance = RoleInstance {
        role_id: Uuid::new_v4(),
        session_id: Uuid::new_v4(),
    };
    let (mut worker, _presence) = transport.bind_worker(WorkerBinding {
        pool: Pool::default(),
        instance,
        capabilities: ["sum/v1".into()].into(),
    })?;
    let call = Call {
        pool: Pool::default(),
        authority: RoleInstance {
            role_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
        },
        worker: instance,
        attempt: Attempt::from_parts("operation-1".into(), "attempt-1".into()),
        capability: "sum/v1".into(),
        payload: Sum(vec![1, 2, 3]),
    };
    let (report, ()) = tokio::join!(transport.deliver(call, Duration::from_secs(1)), async {
        let incoming = worker.receive().await.unwrap().unwrap();
        let total = Total(incoming.call.payload.0.iter().sum());
        incoming
            .reply
            .send(Report::new("report-1".into(), incoming.call.attempt, total))
            .await
            .unwrap();
    });
    assert_eq!(report?.result().0, 6);
    worker.stop().await?;
    Ok(())
}
