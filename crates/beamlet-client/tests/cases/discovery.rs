use crate::support;
use crate::support::{Event, instance};
use beamlet_api::{AuthorityInfo, DiscoveryReply, Pool, Uuid};
use beamlet_client::{Client, Error, Options};
use std::{collections::BTreeSet, time::Duration};
use tokio::sync::mpsc;

/// 装配使用受控传输的 Client 和事件观察端。
///
/// # 返回值
///
/// 二元组 `(Client, 传输事件接收端)`。
fn start() -> (
    Client<support::Transport>,
    mpsc::UnboundedReceiver<Event<String, String>>,
) {
    let (transport, events) = support::pair();
    (Client::new(transport, Options::default()).unwrap(), events)
}
/// 把测试名称列表解析为去重的 pool 集合。
///
/// # 参数
///
/// - `names`：要转换为 pool 标识的测试名称列表。
///
/// # 返回值
///
/// `BTreeSet<Pool>`；任一名称非法时 panic。
fn pools(names: &[&str]) -> BTreeSet<Pool> {
    names.iter().map(|name| name.parse().unwrap()).collect()
}
/// 验证发现过滤无效回复，按会话合并 pools，并在窗口结束释放入口。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
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
/// 验证发现总期限包含准备与收集，EOF 可以提前完成。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
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

/// 验证取消发现会释放准备阶段或收集阶段持有的资源。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
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
    /// 连续立即返回带空请求身份的无效发现回复。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `FloodInbox` 的可变引用。
    ///
    /// # 返回值
    ///
    /// 始终返回 `Ok(Some(DiscoveryReply))` 并增加轮询计数。
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
    /// 创建可持续产生无效回复的发现端点。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Flood` 的共享引用。
    /// - `_`：为满足测试接口签名保留的占位参数，本实现不读取该值。
    /// - `_`：为满足测试接口签名保留的占位参数，本实现不读取该值。
    ///
    /// # 返回值
    ///
    /// 返回 `Ok(FloodInbox)`，与测试共享计数器。
    async fn discover(
        &self,
        _: &BTreeSet<Pool>,
        _: Uuid,
    ) -> Result<FloodInbox, beamlet_api::TransportError> {
        Ok(FloodInbox(self.0.clone()))
    }
    /// 阻止发现专用测试误调用提交接口。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Flood` 的共享引用。
    /// - `_`：为满足测试接口签名保留的占位参数，本实现不读取该值。
    /// - `_`：为满足测试接口签名保留的占位参数，本实现不读取该值。
    ///
    /// # 返回值
    ///
    /// 不返回；一旦调用即 panic，说明测试流程违反约定。
    async fn request(
        &self,
        _: beamlet_api::Command,
        _: Duration,
    ) -> Result<beamlet_api::Reply, beamlet_api::TransportError> {
        unreachable!("discovery never submits commands")
    }
}

/// 验证持续立即返回的无效发现回复不饿死计时器或其他任务。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
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
