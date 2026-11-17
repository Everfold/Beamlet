//! 进程内定向消息传输, 不持有角色选择、租约或 Operation 状态.
mod authority;
mod worker;

pub use authority::{AuthorityEndpoint, Discovery};
pub use worker::{PresenceEndpoint, WorkerEndpoint};

/// 同一次 Worker 绑定的调用入口和公告通道, 可分别驱动.
pub type WorkerEndpoints<P = String, R = String> = (WorkerEndpoint<P, R>, PresenceEndpoint<P, R>);

use beamlet_api::{
    AuthorityEvent, AuthorityInfo, Connection, ConnectionView, Incoming, Pool, TransportError,
    WorkerBinding,
};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::{mpsc, watch};

type Key = (Pool, beamlet_api::RoleInstance);
type AuthoritySend<P, R> = mpsc::Sender<AuthorityEvent<P, R>>;
struct WorkerRoute<P, R> {
    binding: WorkerBinding,
    sender: mpsc::Sender<Incoming<P, R>>,
    probes: watch::Sender<u64>,
}
struct Routes<P, R> {
    authorities: BTreeMap<Key, AuthoritySend<P, R>>,
    workers: BTreeMap<Key, WorkerRoute<P, R>>,
}

/// 一个隔离的进程内消息空间; clone 共享空间, new 创建独立空间.
pub struct Local<P = String, R = String> {
    routes: Arc<Mutex<Routes<P, R>>>,
    capacity: usize,
}
impl<P, R> Clone for Local<P, R> {
    fn clone(&self) -> Self {
        Self {
            routes: self.routes.clone(),
            capacity: self.capacity,
        }
    }
}
impl<P, R> std::fmt::Debug for Local<P, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Local")
            .field("capacity", &self.capacity)
            .finish_non_exhaustive()
    }
}
impl<P: Send + 'static, R: Send + 'static> Local<P, R> {
    /// 创建本地消息空间, 每个绑定入口使用给定的缓冲容量.
    ///
    /// # Errors
    /// 容量为 0 或超过 Tokio 通道范围时返回错误.
    pub fn new(capacity: usize) -> Result<Self, TransportError> {
        if capacity == 0 || capacity > tokio::sync::Semaphore::MAX_PERMITS {
            return Err(TransportError::new("invalid local channel capacity"));
        }
        Ok(Self {
            routes: Arc::new(Mutex::new(Routes {
                authorities: BTreeMap::new(),
                workers: BTreeMap::new(),
            })),
            capacity,
        })
    }
    /// 本地空间在对象存活期间始终连接, 端点另行关闭.
    pub fn connection(&self) -> ConnectionView {
        ConnectionView::new(|| Connection {
            epoch: 1,
            connected: true,
        })
    }
    /// 绑定实例的控制入口, 不自动回答发现或查询.
    ///
    /// # Errors
    /// 空身份、空 pools 或重复绑定时返回错误, 不覆盖原入口.
    pub fn bind_authority(
        &self,
        info: &AuthorityInfo,
    ) -> Result<AuthorityEndpoint<P, R>, TransportError> {
        authority::bind(self.clone(), info)
    }
    /// 绑定显式 Worker 会话及能力, 返回调用和公告通道.
    ///
    /// # Errors
    /// 身份、能力或重复绑定无效时返回错误.
    pub fn bind_worker(
        &self,
        binding: WorkerBinding,
    ) -> Result<WorkerEndpoints<P, R>, TransportError> {
        worker::bind(self.clone(), binding)
    }
    /// 本地发送直接入队, 没有额外刷新缓冲.
    pub async fn flush(&self) -> Result<(), TransportError> {
        Ok(())
    }
}

fn invalid_deadline(deadline: Duration) -> bool {
    deadline.is_zero() || std::time::Instant::now().checked_add(deadline).is_none()
}
