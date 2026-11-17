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
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
pub fn pair<P, R>() -> (Transport<P, R>, mpsc::UnboundedReceiver<Event<P, R>>) {
    let (send, receive) = mpsc::unbounded_channel();
    (Transport(send), receive)
}
pub struct Discovery(mpsc::UnboundedReceiver<DiscoveryReply>);
impl DiscoveryInbox for Discovery {
    async fn receive(&mut self) -> Result<Option<DiscoveryReply>, TransportError> {
        Ok(self.0.recv().await)
    }
}
impl<P: Send + 'static, R: Send + 'static> ClientTransport<P, R> for Transport<P, R> {
    type Discovery = Discovery;
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
pub fn instance(base: u128) -> RoleInstance {
    RoleInstance {
        role_id: Uuid::from_u128(base),
        session_id: Uuid::from_u128(base + 1),
    }
}
