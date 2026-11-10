//! 原 Report 的有界易失交付, 不创建执行授权或推断 Authority 的采用决定

use std::{collections::HashMap, future::Future, time::Duration};

use crate::{
    ExecutionBinding,
    id::{ReportId, RoleInstanceId},
    model::{Delivery, Message, SubmitReport},
    state::RuleError,
};
use thiserror::Error;
use tokio::time::{Instant, timeout};

/// 每份原证据最多发出三次发送项, 不表示执行次数
pub const MAX_SENDS: u8 = 3;
/// 每次匹配等待及发送共用的预算
pub const SEND_TIMEOUT: Duration = Duration::from_millis(500);
const RETRY_DELAY: Duration = Duration::from_secs(1);
const OBSERVATION_TIMEOUT: Duration = Duration::from_secs(5);

/// 显式提供的去重及待发记录上限
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    /// 所有记录数量, 包含已确认和耗尽记录
    pub max_records: usize,
    /// 同时保留待发字节的记录数量
    pub max_pending: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_records: 1024,
            max_pending: 256,
        }
    }
}

/// 收录回执必须匹配的五项身份, 不是网络认证
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Binding {
    /// 原 Authority, Worker, Operation 和 Attempt
    pub execution: ExecutionBinding,
    /// 原证据身份
    pub report_id: ReportId,
}

/// 交付观察不代表任务成功, 证据被采用或已持久化
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Status {
    /// 在原预算内等待收录回执
    Active,
    /// 预算耗尽, 仍可接受迟到回执
    Unconfirmed,
    /// 已收到完整匹配的收录回执
    Confirmed,
}

/// 原证据的交付进度副本
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Progress {
    /// 原证据的完整身份
    pub binding: Binding,
    /// 已发出的发送项数量, 发送失败也计数
    pub sends: u8,
    /// 当前收录观察
    pub status: Status,
}

/// 单次发送关联, 旧完成通知不能释放后续发送的占用
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Ticket {
    binding: Binding,
    number: u8,
}

impl Ticket {
    /// 原证据的完整身份
    pub fn binding(self) -> Binding {
        self.binding
    }
    /// 该证据的发送序号
    pub fn number(self) -> u8 {
        self.number
    }
}

/// 一次性发送项, 拥有固定的原字节和目标 Authority
#[derive(Debug)]
pub struct Transmission {
    ticket: Ticket,
    bytes: Vec<u8>,
}

impl Transmission {
    /// 在发送完成或取消后用此关联通知 Outbox::sent
    pub fn ticket(&self) -> Ticket {
        self.ticket
    }

    /// 限时调用宿主的匹配等待与发送函数, 不创建连接或后台任务
    ///
    /// 宿主管理此 future, 并在成功, 失败或取消后调用 Outbox::sent.
    /// 已交给传输层的消息不保证可撤销.
    /// # 错误
    /// 传输失败或 500ms 期限到达, 均不能证明 Authority 未收录
    pub async fn send<F, Fut, E>(self, send: F) -> Result<(), SendError<E>>
    where
        F: FnOnce(RoleInstanceId, Vec<u8>) -> Fut,
        Fut: Future<Output = Result<(), E>>,
    {
        timeout(SEND_TIMEOUT, async {
            send(
                self.ticket.binding.execution.authority_instance_id,
                self.bytes,
            )
            .await
        })
        .await
        .map_err(|_| SendError::TimedOut)?
        .map_err(SendError::Transport)
    }
}

/// 宿主负责执行及展示的动作, 库不执行 I/O
#[derive(Debug)]
pub enum Event {
    /// 在宿主的发送任务集合中执行
    Send(Transmission),
    /// 释放待发字节后报告一次耗尽, 仍计入未确认数量
    Exhausted(Progress),
}

#[derive(Debug)]
struct Pending {
    bytes: Vec<u8>,
    next: Instant,
    deadline: Instant,
}

#[derive(Debug)]
struct Entry<O, E> {
    evidence: SubmitReport<O, E>,
    progress: Progress,
    pending: Option<Pending>,
    in_flight: Option<u8>,
}

/// 显式绑定 Worker 及其 Authority 的交付箱, 不拥有执行器或传输
#[derive(Debug)]
pub struct Outbox<O, E> {
    worker: RoleInstanceId,
    authority: RoleInstanceId,
    entries: HashMap<ReportId, Entry<O, E>>,
    limits: Limits,
}

impl<O: Clone + Eq, E: Clone + Eq> Outbox<O, E> {
    /// 构造空交付记录, 不启动时钟任务或基础设施
    /// # 错误
    /// 任一记录上限为零时返回 InvalidLimits
    pub fn new(
        worker: RoleInstanceId,
        authority: RoleInstanceId,
        limits: Limits,
    ) -> Result<Self, Error> {
        if limits.max_records == 0 || limits.max_pending == 0 {
            return Err(Error::InvalidLimits);
        }
        Ok(Self {
            worker,
            authority,
            entries: HashMap::new(),
            limits,
        })
    }

    /// 为尚未完成的执行预留记录, 宿主在同一串行边界接纳新执行
    ///
    /// pending_execution 是将来各自产生一份 Report 的已接纳执行数量.
    /// 此检查不创建预留令牌; 调用方须保持该计数与执行器一致.
    pub fn has_capacity(&self, pending_execution: usize) -> bool {
        self.entries.len().saturating_add(pending_execution) < self.limits.max_records
            && self
                .entries
                .values()
                .filter(|entry| entry.pending.is_some())
                .count()
                .saturating_add(pending_execution)
                < self.limits.max_pending
    }

    /// 返回独立的交付观察, 不刷新期限
    pub fn progress(&self, report: ReportId) -> Option<Progress> {
        self.entries.get(&report).map(|entry| entry.progress)
    }

    /// 返回尚未获得收录确认的数量, 包含预算已耗尽的证据
    pub fn unconfirmed(&self) -> usize {
        self.entries
            .values()
            .filter(|entry| entry.progress.status != Status::Confirmed)
            .count()
    }

    /// 加入 Worker 已缓存的原证据, 仅首次加入时调用编码函数
    ///
    /// 保留独立的完整证据用于重复校验, 已确认或耗尽也不允许替换内容.
    /// 相同 ReportId 的合法重复不重编码或刷新预算; 不同证据独立确认.
    /// # 错误
    /// 非 Report, 错实例, 无效证据, 身份/内容冲突, 容量或编码错误时拒绝
    pub fn retain<P, C>(
        &mut self,
        delivery: &Delivery<P, O, E>,
        encode: impl FnOnce(&Delivery<P, O, E>) -> Result<Vec<u8>, C>,
    ) -> Result<(), RetainError<C>> {
        let Message::SubmitReport(evidence) = &delivery.message else {
            return Err(Error::WrongMessage.into());
        };
        if delivery.source_instance_id != self.worker
            || delivery.target_instance_id != self.authority
        {
            return Err(Error::InvalidBinding.into());
        }
        evidence.validate().map_err(Error::InvalidEvidence)?;
        let binding = Binding {
            execution: ExecutionBinding {
                authority_instance_id: delivery.target_instance_id,
                worker_instance_id: delivery.source_instance_id,
                operation_id: evidence.operation_id,
                attempt_id: evidence.attempt_id,
            },
            report_id: evidence.report_id,
        };
        if let Some(old) = self.entries.get(&binding.report_id) {
            if old.progress.binding != binding || old.evidence != *evidence {
                return Err(Error::Conflict.into());
            }
            return Ok(());
        }
        if !self.has_capacity(0) {
            return Err(Error::Capacity.into());
        }
        let bytes = encode(delivery).map_err(RetainError::Encoding)?;
        let now = Instant::now();
        self.entries.insert(
            binding.report_id,
            Entry {
                evidence: evidence.clone(),
                progress: Progress {
                    binding,
                    sends: 0,
                    status: Status::Active,
                },
                pending: Some(Pending {
                    bytes,
                    next: now,
                    deadline: now + OBSERVATION_TIMEOUT,
                }),
                in_flight: None,
            },
        );
        Ok(())
    }

    /// 验证完整回执绑定, 传输路由与认证由 Adapter 先检查
    /// # 错误
    /// 未知 Report 或任一身份不匹配时拒绝, 不改变记录
    pub fn acknowledge(&mut self, binding: Binding) -> Result<bool, Error> {
        let entry = self
            .entries
            .get_mut(&binding.report_id)
            .ok_or(Error::Unknown)?;
        if entry.progress.binding != binding {
            return Err(Error::InvalidBinding);
        }
        let changed = entry.progress.status != Status::Confirmed;
        entry.progress.status = Status::Confirmed;
        entry.pending = None;
        Ok(changed)
    }

    /// 记录成功, 失败或取消的发送; 重复及旧完成通知不影响后续发送
    pub fn sent(&mut self, ticket: Ticket) -> bool {
        let Some(entry) = self.entries.get_mut(&ticket.binding.report_id) else {
            return false;
        };
        if entry.progress.binding != ticket.binding || entry.in_flight != Some(ticket.number) {
            return false;
        }
        entry.in_flight = None;
        if let Some(pending) = &mut entry.pending {
            pending.next = Instant::now() + RETRY_DELAY;
        }
        true
    }

    /// 取出下一发送或一次性耗尽事件, 取得发送项即计数
    ///
    /// 同一记录在 sent 前不发出下一项; 宿主持续轮询期限并管理发送任务.
    /// 无事件时等待输入或时钟, 不应忙轮询; 确认及耗尽均保留去重记录.
    pub fn next_event(&mut self) -> Option<Event> {
        let now = Instant::now();
        for entry in self.entries.values_mut() {
            let Some(pending) = &entry.pending else {
                continue;
            };
            if now >= pending.deadline
                || (entry.in_flight.is_none()
                    && now >= pending.next
                    && entry.progress.sends >= MAX_SENDS)
            {
                entry.pending = None;
                entry.progress.status = Status::Unconfirmed;
                return Some(Event::Exhausted(entry.progress));
            }
            if entry.in_flight.is_none() && now >= pending.next {
                entry.progress.sends += 1;
                entry.in_flight = Some(entry.progress.sends);
                return Some(Event::Send(Transmission {
                    ticket: Ticket {
                        binding: entry.progress.binding,
                        number: entry.progress.sends,
                    },
                    bytes: pending.bytes.clone(),
                }));
            }
        }
        None
    }
}

/// 原证据交付规则错误, 不表示任务执行失败
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum Error {
    /// 记录上限必须为正
    #[error("Report 交付记录上限必须为正")]
    InvalidLimits,
    /// 仅接受原 Report
    #[error("Report 交付必须使用原证据消息")]
    WrongMessage,
    /// 实例或原证据绑定不符
    #[error("Report 交付完整身份绑定不符")]
    InvalidBinding,
    /// 证据的结果与影响相矛盾
    #[error(transparent)]
    InvalidEvidence(RuleError),
    /// 已有同身份证据的定义不同
    #[error("Report 交付身份或完整证据冲突")]
    Conflict,
    /// 去重或待发记录上限已到
    #[error("Report 交付记录已满")]
    Capacity,
    /// 回执未对应原证据
    #[error("未知 Report 收录回执")]
    Unknown,
}

/// 加入交付记录时的规则或编码错误
#[derive(Debug, Error)]
pub enum RetainError<E> {
    /// 原证据不符合交付记录规则
    #[error(transparent)]
    State(#[from] Error),
    /// Adapter 编码失败
    #[error("Report 编码失败: {0}")]
    Encoding(#[source] E),
}

/// 一次发送的结果, 失败不证明远端未收录
#[derive(Debug, Error)]
pub enum SendError<E> {
    /// 匹配等待与发送总预算到达
    #[error("Report 本次发送超时")]
    TimedOut,
    /// 传输返回错误
    #[error("Report 本次发送失败: {0}")]
    Transport(#[source] E),
}
