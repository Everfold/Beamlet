//! 无服务器的完整调用: Client 发现、查询和提交, Authority 与 Worker 处理自定义数据.
use beamlet_api::{
    AuthorityInbox, AuthorityInfo, Outcome, Pool, PresenceState, RoleInstance, Selection,
    Submission, Task, Uuid,
};
use beamlet_authority::{Authority, Options};
use beamlet_transport_local::Local;
use beamlet_worker::{Capabilities, Worker};
use std::{error::Error, time::Duration};
use tokio::sync::oneshot;

#[derive(Clone)]
struct Numbers(Vec<i64>);
struct Total(i64);

fn instance() -> RoleInstance {
    RoleInstance {
        role_id: Uuid::new_v4(),
        session_id: Uuid::new_v4(),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let transport = Local::new(8)?;
    let info = AuthorityInfo {
        instance: instance(),
        label: Some("本地 Authority".into()),
        pools: [Pool::default()].into(),
    };
    let endpoint = transport.bind_authority(&info)?;
    let authority = Authority::new(info.clone(), Options::default(), endpoint.connection())?;
    let mut capabilities = Capabilities::new();
    capabilities.register("sum/v1", |input: Numbers, _| async move {
        Ok(Total(input.0.iter().sum()))
    })?;
    let worker_id = instance();
    let worker = Worker::builder(worker_id, capabilities).build()?;
    let (inbox, presence) = transport.bind_worker(worker.binding())?;
    let (stop_authority, shutdown_authority) = oneshot::channel();
    let (stop_worker, shutdown_worker) = oneshot::channel();
    let authority = tokio::spawn(authority.run(endpoint, transport.clone(), shutdown_authority));
    let worker = tokio::spawn(worker.run(inbox, presence, shutdown_worker));
    let deadline = Duration::from_secs(3);
    let client = beamlet_client::Client::new(transport, beamlet_client::Options::default())?;
    let found = client.discover(&info.pools).await?;
    assert_eq!(found.len(), 1);
    let target = found[0].instance;

    // 公告与提交是独立交互; 等待 Authority 确认 Worker 可用后再指定它.
    tokio::time::timeout(deadline, async {
        loop {
            let workers = client
                .workers(target, Pool::default(), Some("sum/v1"))
                .await?;
            if workers.iter().any(|view| {
                view.presence.instance == worker_id
                    && !view.stale
                    && view.presence.state == PresenceState::Ready
            }) {
                break;
            }
            tokio::task::yield_now().await;
        }
        Ok::<_, beamlet_client::Error>(())
    })
    .await??;
    let outcome = client
        .submit(
            target,
            Submission {
                pool: Pool::default(),
                worker: Selection::Exact(worker_id),
                task: Task {
                    capability: "sum/v1".into(),
                    payload: Numbers(vec![1, 2, 3]),
                },
            },
        )
        .await?;
    let Outcome::Completed {
        operation_id,
        result,
        ..
    } = outcome
    else {
        return Err("operation did not complete".into());
    };
    println!("{operation_id}: sum = {}", result.0);
    stop_worker.send(()).map_err(|_| "worker stopped early")?;
    worker.await??;
    stop_authority
        .send(())
        .map_err(|_| "authority stopped early")?;
    authority.await??;
    Ok(())
}
