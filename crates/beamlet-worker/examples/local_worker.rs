//! 无配置文件和消息服务器的 Worker, 输入/输出无需 Clone、Debug 或 serde.
use beamlet_api::{Call, Delivery, Pool, RoleInstance, Uuid};
use beamlet_primitives::Attempt;
use beamlet_transport_local::Local;
use beamlet_worker::{Capabilities, Concurrency, Options, Worker};
use std::{error::Error, time::Duration};
use tokio::sync::oneshot;

struct Sum(Vec<i64>);
struct Total(i64);

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let instance = RoleInstance {
        role_id: Uuid::new_v4(),
        session_id: Uuid::new_v4(),
    };
    let mut capabilities = Capabilities::new();
    capabilities.register("sum/v1", |input: Sum, context| async move {
        assert_eq!(context.capability(), "sum/v1");
        Ok(Total(input.0.iter().sum()))
    })?;
    let worker = Worker::builder(instance, capabilities)
        .label("本地求和")
        .options(Options {
            concurrency: Concurrency::Limited(2),
            ..Options::default()
        })
        .build()?;
    let transport = Local::new(8)?;
    let (inbox, presence) = transport.bind_worker(worker.binding())?;
    let (stop, shutdown) = oneshot::channel();
    let running = tokio::spawn(worker.run(inbox, presence, shutdown));
    // 示例模拟 Authority 已签发的授权; 业务处理器只产生结果, 不创建 Attempt/Report.
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
    let report = transport.deliver(call, Duration::from_secs(1)).await?;
    assert_eq!(report.result().0, 6);
    assert_eq!(report.id(), &format!("{}:report:1", instance.session_id));
    stop.send(()).map_err(|_| "worker stopped early")?;
    running.await??;
    Ok(())
}
