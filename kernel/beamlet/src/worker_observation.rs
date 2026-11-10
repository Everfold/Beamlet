//! Worker 公告与心跳的本地观察, 不参与授权或调度

use crate::id::{AuthorityInstanceId, NodeId, NodeInstanceId, WorkerId, WorkerInstanceId};
use std::{collections::BTreeMap, num::NonZeroU64, time::Duration};
use thiserror::Error;
use tokio::time::Instant;

/// 原型每个 Authority 的静态观察槽位上限
pub const MAX_WORKERS: usize = 64;
/// 自建槽或最近有效递增心跳起的可疑阈值
pub const SUSPECT_AFTER: Duration = Duration::from_secs(4);

/// 应用显式选择的观察输入来源, 不等于网络认证
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Source {
    /// 同进程类型化交付
    Local,
    /// 经 Adapter 验证路由的远端消息
    Remote,
}

/// 显式配置的 Worker 实例及允许输入来源
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Slot {
    /// 静态允许的运行实例
    pub instance: WorkerInstanceId,
    /// 该槽位接受的交付来源
    pub source: Source,
}

/// 公告自报的完整归属, 首次记录不证明身份所有权
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Binding {
    /// Node 稳定身份
    pub node_id: NodeId,
    /// Node 本次运行
    pub node_instance_id: NodeInstanceId,
    /// Worker 稳定身份
    pub worker_id: WorkerId,
    /// Worker 本次运行
    pub worker_instance_id: WorkerInstanceId,
    /// 目标 Authority 本次运行
    pub authority_instance_id: AuthorityInstanceId,
}

/// 启动事实与带非零递增序号的心跳
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Signal {
    /// 启动公告不证明在线, 不刷新心跳期限
    Started,
    /// 首份有效心跳可直接建立绑定
    Heartbeat(NonZeroU64),
}

/// Adapter 解码后的类型化输入
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Announcement {
    /// 原实例归属
    pub binding: Binding,
    /// 本次观察输入
    pub signal: Signal,
}

/// 心跳观察, 不表示执行器可用或外部效果停止
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Status {
    /// 尚未收到有效心跳
    Unobserved,
    /// 最近收到过有效递增心跳
    Online,
    /// 本地观察期限已到
    Suspected,
}

/// 独立的只读快照, 读取本身不推进时钟观察
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Snapshot {
    /// 首次合法输入建立的不可替换归属
    pub binding: Option<Binding>,
    /// 最近有效心跳序号
    pub sequence: Option<NonZeroU64>,
    /// 最近一次 record 或 poll_expired 后的状态
    pub status: Status,
}

/// 有效心跳引起的状态变化, 用于宿主展示
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Transition {
    /// 从未观察或可疑变为在线
    Online,
    /// 宿主尚未轮询到期, 本次递增心跳前已经超时, 随后恢复在线
    ExpiredThenOnline,
}

/// 一次有效输入引起的变化, 重复启动可返回无变化
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Change {
    /// 本次首次记录完整归属
    pub binding_added: bool,
    /// 只有观察变化时才返回转换
    pub transition: Option<Transition>,
}

#[derive(Debug)]
struct Entry {
    source: Source,
    snapshot: Snapshot,
    last_seen: Instant,
}

/// 有界易失观察表, 构造不创建连接, 调度器或后台时钟
#[derive(Debug)]
pub struct Observations {
    authority: AuthorityInstanceId,
    workers: BTreeMap<WorkerInstanceId, Entry>,
}

impl Observations {
    /// 按明确配置预留槽位, 不允许公告新增实例
    /// # 错误
    /// 超过 64 项或配置重复实例时拒绝
    pub fn new(
        authority: AuthorityInstanceId,
        slots: impl IntoIterator<Item = Slot>,
    ) -> Result<Self, Error> {
        let mut workers = BTreeMap::new();
        let now = Instant::now();
        for slot in slots {
            if workers.contains_key(&slot.instance) {
                return Err(Error::DuplicateInstance);
            }
            if workers.len() == MAX_WORKERS {
                return Err(Error::Capacity);
            }
            workers.insert(
                slot.instance,
                Entry {
                    source: slot.source,
                    snapshot: Snapshot {
                        binding: None,
                        sequence: None,
                        status: Status::Unobserved,
                    },
                    last_seen: now,
                },
            );
        }
        Ok(Self { authority, workers })
    }

    /// 此观察表绑定的 Authority 运行实例
    pub fn authority(&self) -> AuthorityInstanceId {
        self.authority
    }

    /// 查询独立快照, 不刷新期限或改变观察
    pub fn snapshot(&self, worker: WorkerInstanceId) -> Option<Snapshot> {
        self.workers.get(&worker).map(|entry| entry.snapshot)
    }

    /// 原子校验并合并输入, 只用接收方单调时钟判断期限
    /// # 错误
    /// 错 Authority, 未配置实例, 输入来源或归属冲突, 重复/倒序心跳均拒绝
    pub fn record(&mut self, announcement: Announcement, source: Source) -> Result<Change, Error> {
        let binding = announcement.binding;
        if binding.authority_instance_id != self.authority {
            return Err(Error::WrongAuthority);
        }
        let entry = self
            .workers
            .get_mut(&binding.worker_instance_id)
            .ok_or(Error::UnknownInstance)?;
        if entry.source != source {
            return Err(Error::WrongSource);
        }
        if entry.snapshot.binding.is_some_and(|old| old != binding) {
            return Err(Error::BindingConflict);
        }
        let mut transition = None;
        if let Signal::Heartbeat(sequence) = announcement.signal {
            if entry.snapshot.sequence.is_some_and(|old| sequence <= old) {
                return Err(Error::StaleHeartbeat);
            }
            let now = Instant::now();
            transition = match entry.snapshot.status {
                Status::Online if now.duration_since(entry.last_seen) >= SUSPECT_AFTER => {
                    Some(Transition::ExpiredThenOnline)
                }
                Status::Online => None,
                _ => Some(Transition::Online),
            };
            entry.last_seen = now;
            entry.snapshot.sequence = Some(sequence);
            entry.snapshot.status = Status::Online;
        }
        let binding_added = entry.snapshot.binding.is_none();
        entry.snapshot.binding = Some(binding);
        Ok(Change {
            binding_added,
            transition,
        })
    }

    /// 推进一项到期观察, 返回刚变为可疑的实例; 宿主轮询至 None
    ///
    /// 每次转换仅返回一次, 不删除归属或序号. 计时检查由宿主驱动,
    /// 无硬实时保证; 不生成执行授权或推断退出.
    pub fn poll_expired(&mut self) -> Option<WorkerInstanceId> {
        let now = Instant::now();
        for (worker, entry) in &mut self.workers {
            if entry.snapshot.status != Status::Suspected
                && now.duration_since(entry.last_seen) >= SUSPECT_AFTER
            {
                entry.snapshot.status = Status::Suspected;
                return Some(*worker);
            }
        }
        None
    }
}

/// 观察输入拒绝原因, 失败不会修改已有观察
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum Error {
    /// 超出原型观察槽位上限
    #[error("Worker 公告观察数量超过上限")]
    Capacity,
    /// 配置含重复实例
    #[error("Worker 公告观察实例重复")]
    DuplicateInstance,
    /// 消息目标与观察表不符
    #[error("Worker 生命周期消息属于其他 Authority 实例")]
    WrongAuthority,
    /// 公告不能创建新的受信槽位
    #[error("Worker 生命周期消息实例不在配置列表中")]
    UnknownInstance,
    /// 本地与远端输入不得混用
    #[error("Worker 生命周期消息输入来源与配置不符")]
    WrongSource,
    /// 同运行实例的归属不能变化
    #[error("Worker 生命周期消息与已保留的实例归属冲突")]
    BindingConflict,
    /// 重放不能刷新期限或恢复在线
    #[error("Worker 心跳重复或倒序, 不刷新观察时间")]
    StaleHeartbeat,
}
