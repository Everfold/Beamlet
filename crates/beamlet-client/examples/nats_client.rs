//! 独立 SDK 调用示例: cargo run -p beamlet-client --example nats_client -- nats://127.0.0.1:4222
use beamlet_api::{Outcome, Pool, PresenceState, Selection, Submission, Task};
use beamlet_client::{Client, Options};
use beamlet_transport_nats::Nats;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "nats://127.0.0.1:4222".into());
    let transport = Nats::connect(&url, Default::default()).await?;
    let client = Client::new(transport, Options::default())?;
    let pool = Pool::default();
    let authorities = client.discover(&[pool.clone()].into()).await?;
    let authority = authorities.first().ok_or("no authority in default pool")?;
    let workers = client
        .workers(authority.instance, pool.clone(), Some("text.reverse/v1"))
        .await?;
    let worker = workers
        .iter()
        .find(|view| !view.stale && view.presence.state == PresenceState::Ready)
        .ok_or("no ready text.reverse/v1 worker")?;
    let outcome = client
        .submit(
            authority.instance,
            Submission {
                pool,
                worker: Selection::Exact(worker.presence.instance),
                task: Task {
                    capability: "text.reverse/v1".into(),
                    payload: "Hello Beamlet".into(),
                },
            },
        )
        .await?;
    println!("{outcome:?}");
    match outcome {
        Outcome::Completed { .. } => Ok(()),
        _ => Err("submission did not complete".into()),
    }
}
