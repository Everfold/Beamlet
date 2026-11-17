//! 嵌入单个 Worker, 从调用方接收连接参数, 不依赖 node 或配置加载机制.
use beamlet_api::{RoleInstance, Uuid};
use beamlet_transport_nats::{Nats, Options};
use beamlet_worker::{Capabilities, Worker};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "nats://127.0.0.1:4222".into());
    let mut capabilities = Capabilities::new();
    capabilities.register("echo/v1", |text: String, _| async move { Ok(text) })?;
    // 应用自行提供身份; 此示例是临时 Worker, 不保存固定身份.
    let worker = Worker::builder(
        RoleInstance {
            role_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
        },
        capabilities,
    )
    .label("embedded echo")
    .build()?;
    let transport = Nats::connect(&url, Options::default()).await?;
    let (inbox, presence) = transport.bind_worker(worker.binding()).await?;
    worker.run(inbox, presence, tokio::signal::ctrl_c()).await?;
    transport.flush().await?;
    Ok(())
}
