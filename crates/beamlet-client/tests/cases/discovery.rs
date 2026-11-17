use crate::support;
use crate::support::{Event, instance};
use beamlet_api::{AuthorityInfo, DiscoveryReply, Pool, Uuid};
use beamlet_client::{Client, Error, Options};
use std::{collections::BTreeSet, time::Duration};
use tokio::sync::mpsc;

fn start() -> (
    Client<support::Transport>,
    mpsc::UnboundedReceiver<Event<String, String>>,
) {
    let (transport, events) = support::pair();
    (Client::new(transport, Options::default()).unwrap(), events)
}
fn pools(names: &[&str]) -> BTreeSet<Pool> {
    names.iter().map(|name| name.parse().unwrap()).collect()
}
#[tokio::test(start_paused = true)]
async fn discovery_filters_merges_sessions_and_releases_inbox_at_window_end() {
    let (client, mut events) = start();
    let query = tokio::spawn(async move { client.discover(&pools(&["default", "other"])).await });
    let Event::Discover {
        pools: requested,
        id,
        setup,
    } = events.recv().await.unwrap()
    else {
        panic!("expected discovery")
    };
    assert_eq!(requested, pools(&["default", "other"]));
    assert!(!id.is_nil());
    let (send, receive) = mpsc::unbounded_channel();
    setup.send(receive).unwrap();
    for (request_id, instance, names) in [
        (Uuid::new_v4(), instance(1), vec!["default"]),
        (id, instance(0), vec!["default"]),
        (id, instance(5), vec!["unrelated"]),
        (id, instance(1), vec!["default"]),
        (id, instance(1), vec!["other", "extra"]),
        (
            id,
            beamlet_api::RoleInstance {
                session_id: Uuid::new_v4(),
                ..instance(1)
            },
            vec!["default"],
        ),
    ] {
        send.send(DiscoveryReply {
            request_id,
            authority: AuthorityInfo {
                instance,
                label: None,
                pools: pools(&names),
            },
        })
        .unwrap();
    }
    let found = query.await.unwrap().unwrap();
    assert_eq!(found.len(), 2);
    assert_eq!(
        found
            .iter()
            .find(|info| info.instance == instance(1))
            .unwrap()
            .pools,
        pools(&["default", "other", "extra"])
    );
    assert!(send.is_closed(), "window end retained discovery receiver");
    assert!(events.try_recv().is_err(), "discovery retried");
}
#[tokio::test(start_paused = true)]
async fn total_deadline_covers_setup_and_collection_and_eof_can_end_early() {
    for setup_delay in [Duration::from_secs(11), Duration::from_millis(9500)] {
        let (client, mut events) = start();
        let query = tokio::spawn(async move { client.discover(&pools(&["default"])).await });
        let Event::Discover { setup, .. } = events.recv().await.unwrap() else {
            panic!("expected discovery")
        };
        tokio::time::advance(setup_delay).await;
        let (send, receive) = mpsc::unbounded_channel();
        if setup_delay.as_secs() > 10 {
            assert!(matches!(query.await.unwrap(), Err(Error::Timeout)));
            assert!(setup.is_closed());
        } else {
            setup.send(receive).unwrap();
            assert!(matches!(query.await.unwrap(), Err(Error::Timeout)));
            assert!(send.is_closed());
        }
        assert!(events.try_recv().is_err());
    }
    let (client, mut events) = start();
    let before = tokio::time::Instant::now();
    let query = tokio::spawn(async move { client.discover(&pools(&["default"])).await });
    let Event::Discover { setup, .. } = events.recv().await.unwrap() else {
        panic!("expected discovery")
    };
    let (send, receive) = mpsc::unbounded_channel();
    setup.send(receive).unwrap();
    drop(send);
    assert!(query.await.unwrap().unwrap().is_empty());
    assert_eq!(tokio::time::Instant::now(), before);
}

#[tokio::test]
async fn cancelled_discovery_releases_setup_and_collection() {
    for after_setup in [false, true] {
        let (client, mut events) = start();
        let query = tokio::spawn(async move { client.discover(&pools(&["default"])).await });
        let Event::Discover { setup, .. } = events.recv().await.unwrap() else {
            panic!("expected discovery");
        };
        if after_setup {
            let (send, receive) = mpsc::unbounded_channel();
            setup.send(receive).unwrap();
            tokio::task::yield_now().await;
            query.abort();
            assert!(query.await.unwrap_err().is_cancelled());
            assert!(send.is_closed());
        } else {
            query.abort();
            assert!(query.await.unwrap_err().is_cancelled());
            assert!(setup.is_closed());
        }
        assert!(events.try_recv().is_err(), "cancellation retried discovery");
    }
}

#[derive(Clone)]
struct Flood(std::sync::Arc<std::sync::atomic::AtomicUsize>);
struct FloodInbox(std::sync::Arc<std::sync::atomic::AtomicUsize>);
impl beamlet_api::DiscoveryInbox for FloodInbox {
    async fn receive(&mut self) -> Result<Option<DiscoveryReply>, beamlet_api::TransportError> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(Some(DiscoveryReply {
            request_id: Uuid::nil(),
            authority: AuthorityInfo {
                instance: instance(1),
                label: None,
                pools: pools(&["default"]),
            },
        }))
    }
}
impl beamlet_api::ClientTransport for Flood {
    type Discovery = FloodInbox;
    async fn discover(
        &self,
        _: &BTreeSet<Pool>,
        _: Uuid,
    ) -> Result<FloodInbox, beamlet_api::TransportError> {
        Ok(FloodInbox(self.0.clone()))
    }
    async fn request(
        &self,
        _: beamlet_api::Command,
        _: Duration,
    ) -> Result<beamlet_api::Reply, beamlet_api::TransportError> {
        unreachable!("discovery never submits commands")
    }
}

#[tokio::test(start_paused = true)]
async fn continuously_ready_invalid_replies_do_not_starve_timers_or_other_tasks() {
    let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let client = Client::new(Flood(count.clone()), Options::default()).unwrap();
    let query = tokio::spawn(async move { client.discover(&pools(&["default"])).await });
    while count.load(std::sync::atomic::Ordering::Relaxed) == 0 {
        tokio::task::yield_now().await;
    }
    tokio::time::advance(Options::default().discovery_window).await;
    assert!(query.await.unwrap().unwrap().is_empty());
}
