//! 将显式绑定的 Zenoh 会话与目标接入通用远端提交或查询流程

use crate::{Error, Sender, Transport};

/// 借用既有 Transport 的专用回复队列, 不持有或关闭共享连接
///
/// bind 的独占借用防止另一 Client 同时消费该队列. 应用可以事先通过
/// Transport::subscribe 建立共享 Session 的独立角色订阅, 它们不受此绑定影响.
///
/// # 示例
///
/// ```no_run
/// use beamlet::remote::Client;
/// use beamlet_agent::testing::{hello::Task, submission::HelloClientCodec};
/// use beamlet_message_zenoh::{Limits, ClientTransport, Transport};
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut connection = Transport::connect(
///     "tcp/127.0.0.1:7447", "beamlet/v0/console/example/inbox", Limits::default(),
/// ).await?;
/// let channel = ClientTransport::bind(&mut connection, "beamlet/v0/node/inbox/example").await?;
/// let mut client = Client::new(channel, HelloClientCodec::default(), None);
/// let observed = client.submit(7.into(), &Task::Hello).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct ClientTransport<'session> {
    transport: &'session Transport,
    sender: Sender<'session>,
}

impl<'session> ClientTransport<'session> {
    /// 在应用提供的连接上绑定明确目标, 不新建网络连接或接收订阅
    ///
    /// # 错误
    /// 目标地址非法或发送端声明失败时返回错误
    pub async fn bind(transport: &'session mut Transport, target: &str) -> Result<Self, Error> {
        let sender = transport.sender(target).await?;
        Ok(Self { transport, sender })
    }

    /// 等待匹配订阅者, 供应用为启动就绪设置独立期限; 不表示业务接纳
    ///
    /// # 错误
    /// 会话关闭或匹配监听失败时返回错误
    pub async fn wait_for_receiver(&self) -> Result<(), Error> {
        self.sender.wait_for_receiver().await
    }
}

impl beamlet::remote::Transport for ClientTransport<'_> {
    type Error = Error;

    async fn send(&self, bytes: &[u8]) -> Result<(), Error> {
        self.sender.wait_for_receiver().await?;
        self.sender.send(bytes.to_vec()).await
    }

    async fn recv(&self) -> Result<Option<Vec<u8>>, Error> {
        match self.transport.recv().await {
            Ok(bytes) => Ok(Some(bytes)),
            Err(Error::UnexpectedDelete | Error::MessageTooLarge) => Ok(None),
            Err(error) => Err(error),
        }
    }
}
