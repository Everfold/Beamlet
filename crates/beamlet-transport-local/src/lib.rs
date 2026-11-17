//! 进程内定向消息传输, 不持有角色选择、租约或 Operation 状态.
/// 本地 Authority 路由与发现/请求交互的通道实现。
mod authority;
/// Worker 调用与公告端点的绑定、交付、探测和接收。
mod worker;

pub use authority::{AuthorityEndpoint, Discovery};
pub use worker::{PresenceEndpoint, WorkerEndpoint};

/// 同一次 Worker 绑定的调用入口和公告通道, 可分别驱动.
///
/// # 泛型参数
///
/// - `P`：业务输入类型，默认 String。
/// - `R`：业务结果类型，默认 String。
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

/// 进程内路由键：pool 与完整角色会话身份，避免跨池或跨会话混用。
type Key = (Pool, beamlet_api::RoleInstance);
/// 向一个 Authority 控制队列发送已解码事件的有界通道。
///
/// # 泛型参数
///
/// - `P`：业务输入类型。
/// - `R`：业务结果类型。
type AuthoritySend<P, R> = mpsc::Sender<AuthorityEvent<P, R>>;
/// 单个 Worker 的本地路由记录，保存接收许可、调用入口和探测通知。
///
/// # 泛型参数
///
/// - `P`：业务输入类型。
/// - `R`：业务结果类型。
struct WorkerRoute<P, R> {
    /// Worker 调用许可的固定快照，包含 pool、角色会话和启用能力。
    binding: WorkerBinding,
    /// 该 Worker 已接受调用的有界队列发送端。
    sender: mpsc::Sender<Incoming<P, R>>,
    /// 合并探测请求的序号通知发送端，不为每次探测堆积消息。
    probes: watch::Sender<u64>,
}
/// 同一个本地消息空间的 Authority 与 Worker 路由表。
///
/// # 泛型参数
///
/// - `P`：业务输入类型。
/// - `R`：业务结果类型。
struct Routes<P, R> {
    /// Authority 路由键到控制事件发送端的映射。
    authorities: BTreeMap<Key, AuthoritySend<P, R>>,
    /// Worker 路由键到接收许可和调用入口的映射。
    workers: BTreeMap<Key, WorkerRoute<P, R>>,
}

/// 一个隔离的进程内消息空间; clone 共享空间, new 创建独立空间.
///
/// 数据按值传递；调用队列有界，发现与公告允许丢失。
///
/// # 泛型参数
///
/// - `P`：业务输入类型，默认 String。
/// - `R`：业务结果类型，默认 String。
pub struct Local<P = String, R = String> {
    /// 本消息空间的共享路由表，Clone 继续引用同一份状态。
    routes: Arc<Mutex<Routes<P, R>>>,
    /// 创建各本地有界通道时使用的容量。
    capacity: usize,
}
impl<P, R> Clone for Local<P, R> {
    /// 克隆本地传输句柄，共享消息空间而不创建新路由表。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Local<P, R>` 的共享引用。
    ///
    /// # 返回值
    ///
    /// 与原句柄共享 routes 和容量设置的 Local。
    fn clone(&self) -> Self {
        Self {
            routes: self.routes.clone(),
            capacity: self.capacity,
        }
    }
}
impl<P, R> std::fmt::Debug for Local<P, R> {
    /// 向格式化目标写入 `Local<P, R>` 的调试信息。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Local<P, R>` 的共享引用。
    /// - `f`：接收格式化文本的写入目标。
    ///
    /// # 返回值
    ///
    /// 写入成功返回 `Ok(())`；格式化目标拒绝写入时返回 `fmt::Error`。
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Local")
            .field("capacity", &self.capacity)
            .finish_non_exhaustive()
    }
}
impl<P: Send + 'static, R: Send + 'static> Local<P, R> {
    /// 创建一个独立且有界的进程内消息空间。
    ///
    /// # 参数
    ///
    /// - `capacity`：有界接收队列的容量，必须大于零且不超过 Tokio 最大许可数。
    ///
    /// # 返回值
    ///
    /// 成功返回 Local；容量为零或超出范围返回 TransportError。
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
    /// 取得能够持续读取当前连接状态的视图。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Local<P, R>` 的共享引用。
    ///
    /// # 返回值
    ///
    /// 连接视图句柄；后续查询反映连接当前世代和状态。
    pub fn connection(&self) -> ConnectionView {
        ConnectionView::new(|| Connection {
            epoch: 1,
            connected: true,
        })
    }
    /// 在本地消息空间为 Authority 的各个 pool 注册控制入口。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Local<P, R>` 的共享引用。
    /// - `info`：Authority 的角色/会话身份、标签及其加入的 pools。
    ///
    /// # 返回值
    ///
    /// 成功返回 AuthorityEndpoint；身份/pools 非法或已有相同绑定返回 TransportError。
    ///
    /// # Errors
    /// 空身份、空 pools 或重复绑定时返回错误, 不覆盖原入口.
    pub fn bind_authority(
        &self,
        info: &AuthorityInfo,
    ) -> Result<AuthorityEndpoint<P, R>, TransportError> {
        authority::bind(self.clone(), info)
    }
    /// 注册本地 Worker 调用入口和探测/公告端点。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Local<P, R>` 的共享引用。
    /// - `binding`：Worker 的 pool、角色/会话身份和启用能力集合。
    ///
    /// # 返回值
    ///
    /// 成功返回 `(WorkerEndpoint, PresenceEndpoint)`；绑定非法或重复返回 TransportError。
    ///
    /// # Errors
    /// 身份、能力或重复绑定无效时返回错误.
    pub fn bind_worker(
        &self,
        binding: WorkerBinding,
    ) -> Result<WorkerEndpoints<P, R>, TransportError> {
        worker::bind(self.clone(), binding)
    }
    /// 完成本地传输刷新；本地通道不需要额外网络刷新。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Local<P, R>` 的共享引用。
    ///
    /// # 返回值
    ///
    /// 始终返回 `Ok(())`。
    pub async fn flush(&self) -> Result<(), TransportError> {
        Ok(())
    }
}

/// 判断交互期限是否为零或超出时钟表示范围。
///
/// # 参数
///
/// - `deadline`：等待本次回复的最长时长，非绝对时间；超时不取消远端执行。
///
/// # 返回值
///
/// 期限非法返回 `true`，否则返回 `false`。
fn invalid_deadline(deadline: Duration) -> bool {
    deadline.is_zero() || std::time::Instant::now().checked_add(deadline).is_none()
}
