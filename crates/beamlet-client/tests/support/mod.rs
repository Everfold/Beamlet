use beamlet_api::{
    ClientTransport, Command, DiscoveryInbox, DiscoveryReply, Pool, Reply, RoleInstance,
    TransportError, Uuid,
};
use std::{collections::BTreeSet, time::Duration};
use tokio::sync::{mpsc, oneshot};

pub enum Event<P, R> {
    Discover {
        pools: BTreeSet<Pool>,
        id: Uuid,
        setup: oneshot::Sender<mpsc::UnboundedReceiver<DiscoveryReply>>,
    },
    Request {
        command: Command<P>,
        deadline: Duration,
        reply: oneshot::Sender<Result<Reply<R>, TransportError>>,
    },
}
pub struct Transport<P = String, R = String>(mpsc::UnboundedSender<Event<P, R>>);
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
pub struct Discovery(mpsc::UnboundedReceiver<DiscoveryReply>);
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
