//! NATS 与本地输入共用 Authority, Worker task 通过 channel 执行文本倒写.

mod server;
mod worker;

use anyhow::Result;

/// 启动唯一的在线模式, Authority 与 Worker 在同一 Tokio 运行时内协作.
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    server::run().await
}
