//! 多角色嵌入: 两个共享消息空间的 Authority 与两个 Worker, 另一个隔离的角色组.
use beamlet_api::{
    AuthorityInbox, AuthorityInfo, Outcome, Pool, PresenceState, RoleInstance, Selection,
    Submission, Task, Uuid,
};
use beamlet_authority::Authority;
use beamlet_client::Client;
use beamlet_host::{Host, Options};
use beamlet_transport_local::Local;
use beamlet_worker::{Capabilities, Worker};
use std::{collections::BTreeSet, error::Error, time::Duration};
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
    let mut host = Host::new(Options::default())?;
    let mut groups = Vec::new();
    for count in [2, 1] {
        // 每次 new 创建隔离空间, 同一空间的 clone 在角色之间共享.
        let transport = Local::<Numbers, Total>::new(32)?;
        let mut authorities = BTreeSet::new();
        let mut workers = BTreeSet::new();
        for _ in 0..count {
            let info = AuthorityInfo {
                instance: instance(),
                label: None,
                pools: [Pool::default()].into(),
            };
            let endpoint = transport.bind_authority(&info)?;
            let authority =
                Authority::new(info.clone(), Default::default(), endpoint.connection())?;
            let shutdown = host.shutdown();
            host.add(
                info.instance.role_id.to_string(),
                authority.run(endpoint, transport.clone(), shutdown.wait()),
            );
            authorities.insert(info.instance);

            let mut capabilities = Capabilities::new();
            capabilities.register("sum/v1", |numbers: Numbers, _| async move {
                Ok(Total(numbers.0.iter().sum()))
            })?;
            let worker_id = instance();
            let worker = Worker::builder(worker_id, capabilities).build()?;
            let (inbox, presence) = transport.bind_worker(worker.binding())?;
            let shutdown = host.shutdown();
            host.add(
                worker_id.role_id.to_string(),
                worker.run(inbox, presence, shutdown.wait()),
            );
            workers.insert(worker_id);
        }
        groups.push((
            Client::new(transport, Default::default())?,
            authorities,
            workers,
        ));
    }
    let (stop, signal) = oneshot::channel();
    let exercise = async {
        let mut completed = 0;
        for (client, expected_authorities, expected_workers) in groups {
            let authorities = client.discover(&[Pool::default()].into()).await?;
            assert_eq!(
                authorities
                    .iter()
                    .map(|info| info.instance)
                    .collect::<BTreeSet<_>>(),
                expected_authorities
            );
            for authority in authorities {
                loop {
                    let workers = client
                        .workers(authority.instance, Pool::default(), Some("sum/v1"))
                        .await?;
                    let ready: BTreeSet<_> = workers
                        .iter()
                        .filter(|view| !view.stale && view.presence.state == PresenceState::Ready)
                        .map(|view| view.presence.instance)
                        .collect();
                    if ready == expected_workers {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
                for worker in &expected_workers {
                    let outcome = client
                        .submit(
                            authority.instance,
                            Submission {
                                pool: Pool::default(),
                                worker: Selection::Exact(*worker),
                                task: Task {
                                    capability: "sum/v1".into(),
                                    payload: Numbers(vec![1, 2, 3]),
                                },
                            },
                        )
                        .await?;
                    let Outcome::Completed {
                        worker: actual,
                        result,
                        ..
                    } = outcome
                    else {
                        return Err("expected completed operation".into());
                    };
                    assert_eq!(actual, *worker);
                    assert_eq!(result.0, 6);
                    completed += 1;
                }
            }
        }
        Ok::<_, Box<dyn Error>>(completed)
    };
    let (host_result, work_result) = tokio::join!(host.run(signal), async {
        let result = tokio::time::timeout(Duration::from_secs(3), exercise).await;
        let _ = stop.send(());
        result
    });
    host_result?;
    let completed = work_result??;
    println!(
        "3 authorities, 3 workers, 2 isolated transports: {completed} operations completed; all roles drained"
    );
    Ok(())
}
