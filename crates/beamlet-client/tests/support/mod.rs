use beamlet_api::{
    ClientTransport, Command, DiscoveryInbox, DiscoveryReply, Pool, Reply, RoleInstance,
    TransportError, Uuid,
};
use std::{collections::BTreeSet, time::Duration};
use tokio::sync::{mpsc, oneshot};

/// 受控 ClientTransport 发给测试观察者的交互事件。
///
/// # 泛型参数
///
/// - `P`：业务输入类型。
/// - `R`：业务结果类型。
pub enum Event<P, R> {
    /// 要求测试驱动提供发现回复流，允许暂停准备阶段。
    Discover {
        /// 本次发现查询关注的 pools。
        pools: BTreeSet<Pool>,
        /// Client 为发现分配的请求 UUID。
        id: Uuid,
        /// 由测试驱动交回发现回复队列的准备通道。
        setup: oneshot::Sender<mpsc::UnboundedReceiver<DiscoveryReply>>,
    },
    /// 已经送入传输的一次 Authority 命令，等待测试驱动决定回复。
    Request {
        /// 需要保留关联与 payload 的原命令。
        command: Command<P>,
        /// Client 传入的本次请求最长等待时长。
        deadline: Duration,
        /// 把成功回复或传输错误返回给原调用的一次性通道。
        reply: oneshot::Sender<Result<Reply<R>, TransportError>>,
    },
}
/// 可克隆的受控客户端传输，将交互转换为可观测事件。
///
/// # 泛型参数
///
/// - `P`：业务输入类型，默认 String。
/// - `R`：业务结果类型，默认 String。
pub struct Transport<P = String, R = String>(
    /// 交给测试驱动的无界事件发送端，克隆后共享同一观察者。
    mpsc::UnboundedSender<Event<P, R>>,
);
impl<P, R> Clone for Transport<P, R> {
    /// 复制测试传输发送端，共享同一个事件观察通道。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Transport<P, R>` 的共享引用。
    ///
    /// # 返回值
    ///
    /// 新的 Transport 句柄，不克隆业务输入或结果。
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
/// 创建用于控制客户端传输行为的事件通道。
///
/// # 返回值
///
/// 二元组 `(Transport, 事件接收端)`。
pub fn pair<P, R>() -> (Transport<P, R>, mpsc::UnboundedReceiver<Event<P, R>>) {
    let (send, receive) = mpsc::unbounded_channel();
    (Transport(send), receive)
}
/// 接收测试驱动逐条提供的发现回复。
pub struct Discovery(
    /// 由测试驱动控制结束和数据的发现回复队列。
    mpsc::UnboundedReceiver<DiscoveryReply>,
);
impl DiscoveryInbox for Discovery {
    /// 从测试发现队列读取一份回复。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Discovery` 的可变引用。
    ///
    /// # 返回值
    ///
    /// 有回复为 `Ok(Some(reply))`；队列关闭为 `Ok(None)`。
    async fn receive(&mut self) -> Result<Option<DiscoveryReply>, TransportError> {
        Ok(self.0.recv().await)
    }
}
impl<P: Send + 'static, R: Send + 'static> ClientTransport<P, R> for Transport<P, R> {
    /// 测试传输使用的逐条发现回复接收端。
    type Discovery = Discovery;
    /// 向测试观察者发出发现准备事件并等待它提供回复队列。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Transport<P, R>` 的共享引用。
    /// - `pools`：本次发现或登记关注的 pool 集合，按精确名称匹配。
    /// - `id`：由 Client 分配的本次发现请求 UUID。
    ///
    /// # 返回值
    ///
    /// 成功返回 Discovery；观察者或准备通道关闭时返回 TransportError。
    async fn discover(
        &self,
        pools: &BTreeSet<Pool>,
        id: Uuid,
    ) -> Result<Discovery, TransportError> {
        let (setup, receive) = oneshot::channel();
        self.0
            .send(Event::Discover {
                pools: pools.clone(),
                id,
                setup,
            })
            .map_err(|_| TransportError::new("test peer closed"))?;
        Ok(Discovery(receive.await.map_err(TransportError::new)?))
    }
    /// 向测试观察者交出命令，等待它决定本次回复或错误。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Transport<P, R>` 的共享引用。
    /// - `command`：本次 Authority 命令，携带请求 UUID、目标会话及查询/提交数据。
    /// - `deadline`：等待本次回复的最长时长，非绝对时间；超时不取消远端执行。
    ///
    /// # 返回值
    ///
    /// 观察者提供的 Reply 或 TransportError；观察通道关闭也返回 TransportError。
    async fn request(
        &self,
        command: Command<P>,
        deadline: Duration,
    ) -> Result<Reply<R>, TransportError> {
        let (reply, receive) = oneshot::channel();
        self.0
            .send(Event::Request {
                command,
                deadline,
                reply,
            })
            .map_err(|_| TransportError::new("test peer closed"))?;
        receive.await.map_err(TransportError::new)?
    }
}
/// 用相邻数值构造可重复的测试角色和会话 UUID。
///
/// # 参数
///
/// - `base`：用于生成确定性角色/会话 UUID 的起始数值。
///
/// # 返回值
///
/// 角色 UUID 取 base、会话 UUID 取 base 加 1 的 RoleInstance。
pub fn instance(base: u128) -> RoleInstance {
    RoleInstance {
        role_id: Uuid::from_u128(base),
        session_id: Uuid::from_u128(base + 1),
    }
}
