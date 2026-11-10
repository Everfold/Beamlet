//! 原 Attempt 的有界交付记录, 不创建授权, 连接或后台发送任务

use crate::{
    Authority, ExecutionBinding,
    id::{OperationId, ReportId, RoleInstanceId},
    model::{Delivery, DispatchAttempt, Message},
};
use std::{collections::HashMap, future::Future, time::Duration};
use thiserror::Error;
use tokio::time::{Instant, timeout};

/// 原型每份授权最多发送三次, 计数不是执行授权数量
pub const MAX_SENDS: u8 = 3;
/// 一次匹配等待和发送共用的原型预算
pub const SEND_TIMEOUT: Duration = Duration::from_millis(500);
const RETRY_DELAY: Duration = Duration::from_secs(1);
const OBSERVATION_TIMEOUT: Duration = Duration::from_secs(5);

/// 应用显式提供的交付记录上限, 不属于 Operation 的领域容量
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    /// 所有去重记录数量, 包含已确认和耗尽记录
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

/// 对原授权交付的局部观察, 不推断任务执行状态
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Status {
    /// 仍在原预算内等待接纳或证据
    Active,
    /// 预算耗尽后未确认, 仍接受迟到回执
    Unconfirmed,
    /// 匹配的 Worker 回执或 Authority 已收录 Report 确认交付
    Confirmed,
}

/// 可审阅的交付进度副本
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Progress {
    /// 原授权完整绑定
    pub binding: ExecutionBinding,
    /// 已发出的发送项数量, 发送失败也计数
    pub sends: u8,
    /// 当前交付观察
    pub status: Status,
}

/// 单次发送完成的关联, 旧完成通知不能释放后续发送的占用
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Ticket {
    binding: ExecutionBinding,
    number: u8,
}

impl Ticket {
    /// 原执行绑定, 包含目标 Worker
    pub fn binding(self) -> ExecutionBinding {
        self.binding
    }
    /// 本次发送项的序号
    pub fn number(self) -> u8 {
        self.number
    }
}

/// 由驱动发出的一次发送项; 编码字节已固定, 不允许构造新授权
#[derive(Debug)]
pub struct Transmission {
    ticket: Ticket,
    bytes: Vec<u8>,
}

impl Transmission {
    /// 发送结束或被调用方取消后, 用此关联通知 Outbox::sent
    pub fn ticket(&self) -> Ticket {
        self.ticket
    }

    /// 在 500ms 内调用应用提供的传输, 目标和原字节一并交付
    ///
    /// send 必须包含匹配等待及实际发送, 不得留下独立后台重投.
    /// 丢弃 future 不撤销已交给传输层的消息, 调用方仍须按 ticket 记录完成或取消.
    ///
    /// # 错误
    /// 匹配/发送失败或期限到达时返回错误, 不证明 Worker 未接纳
    pub async fn send<F, Fut, E>(self, send: F) -> Result<(), SendError<E>>
    where
        F: FnOnce(RoleInstanceId, Vec<u8>) -> Fut,
        Fut: Future<Output = Result<(), E>>,
    {
        timeout(SEND_TIMEOUT, async {
            send(self.ticket.binding.worker_instance_id, self.bytes).await
        })
        .await
        .map_err(|_| SendError::TimedOut)?
        .map_err(SendError::Transport)
    }
}

/// 宿主应处理的下一项交付动作, 无动作时等待自己的输入或时钟
#[derive(Debug)]
pub enum Event {
    /// 宿主可在自己的任务集合中运行此发送
    Send(Transmission),
    /// 释放待发字节后的未确认观察, 每条记录最多产生一次
    Exhausted(Progress),
}

#[derive(Debug)]
struct Pending {
    bytes: Vec<u8>,
    next: Instant,
    deadline: Instant,
}

#[derive(Debug)]
struct Entry<P> {
    definition: DispatchAttempt<P>,
    progress: Progress,
    pending: Option<Pending>,
    in_flight: Option<u8>,
}

/// 一个 Authority 的易失交付箱, 构造不启动时钟任务或基础设施
#[derive(Debug)]
pub struct Outbox<P> {
    authority: RoleInstanceId,
    entries: HashMap<OperationId, Entry<P>>,
    limits: Limits,
}

impl<P: Clone + Eq> Outbox<P> {
    /// 绑定签发实例及记录上限, 原型交付预算保持固定
    /// # 错误
    /// 任一记录上限为零时返回 InvalidLimits
    pub fn new(authority: RoleInstanceId, limits: Limits) -> Result<Self, Error> {
        if limits.max_records == 0 || limits.max_pending == 0 {
            return Err(Error::InvalidLimits);
        }
        Ok(Self {
            authority,
            entries: HashMap::new(),
            limits,
        })
    }

    /// 是否还能保留新授权; 宿主在接纳新请求前检查并保持同一串行修改边界
    pub fn has_capacity(&self) -> bool {
        self.entries.len() < self.limits.max_records
            && self
                .entries
                .values()
                .filter(|entry| entry.pending.is_some())
                .count()
                < self.limits.max_pending
    }

    /// 返回已有记录的独立观察, 不刷新期限或预留发送资格
    pub fn progress(&self, operation: OperationId) -> Option<Progress> {
        self.entries.get(&operation).map(|entry| entry.progress)
    }

    /// 保留既有授权并仅在首次加入时编码, 重复调用不刷新预算
    ///
    /// 编码与字节上限由应用提供的 Adapter 负责; 完整定义按 P::Eq 比较.
    /// 失败不占用本交付箱的记录, 不改动 Authority 的接纳事实.
    ///
    /// # 错误
    /// 非授权消息, 错实例, 身份/定义冲突, 容量或编码错误时拒绝
    pub fn retain<O, E, C>(
        &mut self,
        dispatch: &Delivery<P, O, E>,
        encode: impl FnOnce(&Delivery<P, O, E>) -> Result<Vec<u8>, C>,
    ) -> Result<(), RetainError<C>> {
        let Message::DispatchAttempt(definition) = &dispatch.message else {
            return Err(Error::WrongMessage.into());
        };
        if dispatch.source_instance_id != self.authority {
            return Err(Error::InvalidBinding.into());
        }
        let binding = ExecutionBinding {
            authority_instance_id: dispatch.source_instance_id,
            worker_instance_id: dispatch.target_instance_id,
            operation_id: definition.operation.operation_id,
            attempt_id: definition.attempt_id,
        };
        if let Some(old) = self.entries.get(&binding.operation_id) {
            if old.progress.binding != binding || old.definition != *definition {
                return Err(Error::Conflict.into());
            }
            return Ok(());
        }
        if self
            .entries
            .values()
            .any(|entry| entry.progress.binding.attempt_id == binding.attempt_id)
        {
            return Err(Error::Conflict.into());
        }
        if !self.has_capacity() {
            return Err(Error::Capacity.into());
        }
        let bytes = encode(dispatch).map_err(RetainError::Encoding)?;
        let now = Instant::now();
        self.entries.insert(
            binding.operation_id,
            Entry {
                definition: definition.clone(),
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

    /// 验证回执的四项绑定并停止后续发送, 传输路由与认证由 Adapter 先检查
    /// # 错误
    /// 未知执行或完整绑定不符时返回错误, 不改变其他记录
    pub fn acknowledge(&mut self, binding: ExecutionBinding) -> Result<bool, Error> {
        let entry = self
            .entries
            .get_mut(&binding.operation_id)
            .ok_or(Error::Unknown)?;
        if entry.progress.binding != binding {
            return Err(Error::InvalidBinding);
        }
        let changed = entry.progress.status != Status::Confirmed;
        entry.progress.status = Status::Confirmed;
        entry.pending = None;
        Ok(changed)
    }

    /// 仅从 Authority 已保留的证据确认原授权; 无交付记录的本地执行返回 false
    /// # 错误
    /// 证据未收录或实例/授权绑定不符时返回错误, 不停止重投
    pub fn reported<O: Clone + Eq, E: Clone + Eq>(
        &mut self,
        authority: &Authority<P, O, E>,
        id: ReportId,
    ) -> Result<bool, Error> {
        let delivery = authority.report(id).ok_or(Error::ReportNotRetained)?;
        if delivery.target_instance_id != self.authority {
            return Err(Error::InvalidBinding);
        }
        let Message::SubmitReport(report) = &delivery.message else {
            unreachable!("Authority 证据记录必须是 Report")
        };
        if !self.entries.contains_key(&report.operation_id) {
            return Ok(false);
        }
        self.acknowledge(ExecutionBinding {
            authority_instance_id: delivery.target_instance_id,
            worker_instance_id: delivery.source_instance_id,
            operation_id: report.operation_id,
            attempt_id: report.attempt_id,
        })
    }

    /// 记录一次发送结束或取消; 成功与错误均等待原间隔, 重复/过期完成不生效
    pub fn sent(&mut self, ticket: Ticket) -> bool {
        let Some(entry) = self.entries.get_mut(&ticket.binding.operation_id) else {
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

    /// 取出下一发送或耗尽事件; 调用本身消耗发送次数, 不执行 I/O
    ///
    /// 同一记录在 sent 前不再发出发送项. 宿主定期调用以检查期限;
    /// 无动作时不应忙轮询. 已耗尽/确认记录继续保留去重事实.
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

/// 授权交付记录错误, 不代表任务失败
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum Error {
    /// 记录上限必须为正
    #[error("Attempt 交付记录上限必须为正")]
    InvalidLimits,
    /// 只接受原 DispatchAttempt
    #[error("Attempt 交付必须使用原授权消息")]
    WrongMessage,
    /// 实例或原授权绑定不符
    #[error("Attempt 交付完整身份绑定不符")]
    InvalidBinding,
    /// 不可变定义或授权身份已被另一执行占用
    #[error("Attempt 交付身份或完整定义冲突")]
    Conflict,
    /// 去重或待发记录上限已到
    #[error("Attempt 交付记录已满")]
    Capacity,
    /// 回执没有对应的原执行
    #[error("未知 Attempt 接纳回执")]
    Unknown,
    /// Report 尚未通过 Authority 校验并收录
    #[error("Authority 尚未收录此 Report")]
    ReportNotRetained,
}

/// 加入交付记录时的规则或编码错误
#[derive(Debug, Error)]
pub enum RetainError<E> {
    /// 原授权不符合交付记录规则
    #[error(transparent)]
    State(#[from] Error),
    /// Adapter 编码失败
    #[error("Attempt 编码失败: {0}")]
    Encoding(#[source] E),
}

/// 一次发送的传输结果, 失败不证明未接纳
#[derive(Debug, Error)]
pub enum SendError<E> {
    /// 本次匹配等待与发送总预算到达
    #[error("Attempt 本次发送超时")]
    TimedOut,
    /// 传输返回错误
    #[error("Attempt 本次发送失败: {0}")]
    Transport(#[source] E),
}
