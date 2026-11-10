//! 公共角色启动与退出事实的单调合并, 不推断在线或当前运行实例

use crate::id::{AuthorityId, NodeId, NodeInstanceId, RoleInstanceId, WorkerId};
use std::collections::BTreeMap;
use thiserror::Error;

/// 角色种类及其稳定身份
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Identity {
    /// 授权方
    Authority(AuthorityId),
    /// 执行方
    Worker(WorkerId),
}

/// 同一运行实例不可替换的自报归属
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Binding {
    /// Node 稳定身份
    pub node_id: NodeId,
    /// Node 本次运行
    pub node_instance_id: NodeInstanceId,
    /// 角色种类与稳定身份
    pub identity: Identity,
    /// 角色本次运行
    pub instance: RoleInstanceId,
}

/// 本地驱动的退出观察, 不代表业务完成或远端收录
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExitOutcome {
    /// 正常排空
    Drained,
    /// 执行或交付未确认
    Unconfirmed,
}

/// 启动与退出事实, 启动不等于当前在线
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Phase {
    /// 已观察到启动
    Started,
    /// 已观察到退出, 后续启动重播不回退此事实
    Exited(ExitOutcome),
}

/// Adapter 解码后的公共通知及观察快照
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Notice {
    /// 原运行实例归属
    pub binding: Binding,
    /// 启动或退出事实
    pub phase: Phase,
}

/// 有界易失通知表, 无通知队列或网络连接
#[derive(Debug)]
pub struct Observations {
    records: BTreeMap<RoleInstanceId, Notice>,
    max_records: usize,
}

impl Observations {
    /// 显式指定记录上限, 满后拒绝新实例而不淘汰旧事实
    /// # 错误
    /// 记录上限为零时拒绝
    pub fn new(max_records: usize) -> Result<Self, Error> {
        if max_records == 0 {
            return Err(Error::InvalidLimit);
        }
        Ok(Self {
            records: BTreeMap::new(),
            max_records,
        })
    }

    /// 查询独立快照; 同一稳定身份的新运行实例各自保留
    pub fn snapshot(&self, instance: RoleInstanceId) -> Option<Notice> {
        self.records.get(&instance).copied()
    }

    /// 原子合并通知, 返回是否新增事实; 路由与认证由 Adapter 先验证
    /// # 错误
    /// 同实例归属或退出结果冲突, 或新增实例超出上限时拒绝
    pub fn record(&mut self, notice: Notice) -> Result<bool, Error> {
        if let Some(old) = self.records.get(&notice.binding.instance) {
            if old.binding != notice.binding {
                return Err(Error::BindingConflict);
            }
            match (old.phase, notice.phase) {
                (Phase::Exited(old), Phase::Exited(new)) if old != new => {
                    return Err(Error::OutcomeConflict);
                }
                (Phase::Exited(_), _) | (Phase::Started, Phase::Started) => return Ok(false),
                (Phase::Started, Phase::Exited(_)) => {}
            }
        } else if self.records.len() == self.max_records {
            return Err(Error::Capacity);
        }
        self.records.insert(notice.binding.instance, notice);
        Ok(true)
    }
}

/// 公共通知合并拒绝原因, 失败不覆盖已有事实
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum Error {
    /// 记录上限必须为正
    #[error("角色观察记录上限必须为正")]
    InvalidLimit,
    /// 不淘汰已有实例来接受新实例
    #[error("Client 生命周期通知记录已满, 不淘汰已有实例")]
    Capacity,
    /// 同实例归属发生变化
    #[error("角色生命周期通知与已保留实例归属冲突")]
    BindingConflict,
    /// 退出结果相互矛盾
    #[error("角色退出结果与已保留终态冲突")]
    OutcomeConflict,
}
