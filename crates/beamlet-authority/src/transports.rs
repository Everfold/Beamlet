//! 异构传输的控制任务与单次交付, 不依赖具体消息后端.
use crate::{
    Request, RunError, control,
    registry::{Registry, Source},
};
use beamlet_api::{
    AuthorityInbox, AuthorityInfo, Call, ConnectionView, Delivery, DeliveryError, WorkerReport,
};
use std::{future::Future, pin::Pin, sync::Arc, time::Duration};
use tokio::{sync::mpsc, task::JoinSet};
use tracing::Instrument;

/// Authority 使用的传输集合; 每项的接收端和交付端必须属于同一消息空间.
///
/// # 泛型参数
/// - `P`: 业务输入类型.
/// - `R`: 业务结果类型.
pub struct Transports<P = String, R = String> {
    /// 按注册顺序固定的传输绑定.
    bindings: Vec<Binding<P, R>>,
}

/// 单个传输的控制入口与交付出口.
struct Binding<P, R> {
    /// 当前连接状态, 仅影响这个传输的租约.
    connection: ConnectionView,
    /// 运行时才启动的控制循环.
    control: Control<P, R>,
    /// 此传输的单次交付函数.
    delivery: Sender<P, R>,
}

/// 控制任务的启动函数.
type Control<P, R> = Box<
    dyn FnOnce(AuthorityInfo, Registry, Source, mpsc::Sender<Request<P, R>>) -> ControlFuture
        + Send,
>;
/// 独立控制任务的运行结果.
type ControlFuture = Pin<Box<dyn Future<Output = Result<(), RunError>> + Send>>;
/// 单次交付的报告结果.
type DeliveryFuture<R> =
    Pin<Box<dyn Future<Output = Result<WorkerReport<R>, DeliveryError>> + Send>>;
/// 擦除具体传输类型的交付函数.
type SendCall<P, R> = dyn Fn(Call<P>, Duration) -> DeliveryFuture<R> + Send + Sync;

/// 按注册顺序共享的交付出口集合.
pub(crate) type Deliveries<P, R> = Arc<[Sender<P, R>]>;

/// 可共享的单次交付出口.
pub(crate) struct Sender<P, R>(Arc<SendCall<P, R>>);
impl<P, R> Sender<P, R> {
    /// 向此出口交付 call, 最多等待 deadline, 返回报告或交付错误.
    pub(crate) async fn deliver(
        &self,
        call: Call<P>,
        deadline: Duration,
    ) -> Result<WorkerReport<R>, DeliveryError> {
        (self.0)(call, deadline).await
    }
}

impl<P, R> Default for Transports<P, R> {
    /// 返回空传输集合, 运行前至少添加一项.
    fn default() -> Self {
        Self {
            bindings: Vec::new(),
        }
    }
}
impl<P, R> std::fmt::Debug for Transports<P, R> {
    /// 向 f 写入传输数量, 返回格式化结果.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Transports")
            .field("len", &self.bindings.len())
            .finish()
    }
}
impl<P: Send + 'static, R: Send + 'static> Transports<P, R> {
    /// 返回空传输集合, 不启动任务.
    pub fn new() -> Self {
        Self::default()
    }

    /// 添加匹配的 endpoint 和 delivery; 所有端点须绑定同一 AuthorityInfo.
    pub fn add(
        &mut self,
        endpoint: impl AuthorityInbox<P, R> + 'static,
        delivery: impl Delivery<P, R>,
    ) {
        self.bindings.push(Binding {
            connection: endpoint.connection(),
            control: Box::new(move |info, registry, source, send| {
                Box::pin(control::control(endpoint, info, registry, source, send))
            }),
            delivery: Sender(Arc::new(move |call, deadline| {
                let delivery = delivery.clone();
                Box::pin(async move { delivery.deliver(call, deadline).await })
            })),
        });
    }

    /// 为 info 和 registry 启动控制任务, 向 send 提交请求, 返回按路由排序的出口.
    pub(crate) fn start(
        self,
        info: AuthorityInfo,
        registry: Registry,
        send: mpsc::Sender<Request<P, R>>,
        controls: &mut JoinSet<Result<(), RunError>>,
    ) -> Result<Deliveries<P, R>, RunError> {
        if self.bindings.is_empty() {
            return Err(RunError::NoTransports);
        }
        let mut deliveries = Vec::with_capacity(self.bindings.len());
        for (id, binding) in self.bindings.into_iter().enumerate() {
            let source = Source {
                id,
                connection: binding.connection,
            };
            controls.spawn(
                (binding.control)(info.clone(), registry.clone(), source, send.clone())
                    .in_current_span(),
            );
            deliveries.push(binding.delivery);
        }
        Ok(deliveries.into())
    }
}
impl<P, R, I, D> From<(I, D)> for Transports<P, R>
where
    P: Send + 'static,
    R: Send + 'static,
    I: AuthorityInbox<P, R> + 'static,
    D: Delivery<P, R>,
{
    /// 将匹配的接收端和交付端 pair 转为单传输集合.
    fn from(pair: (I, D)) -> Self {
        let mut transports = Self::new();
        transports.add(pair.0, pair.1);
        transports
    }
}
