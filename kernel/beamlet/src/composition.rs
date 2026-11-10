//! 角色组装的纯配置与校验, 不生成身份, 构造角色或连接基础设施

use crate::{
    id::{AuthorityId, AuthorityInstanceId, NodeId, RoleInstanceId, WorkerId, WorkerInstanceId},
    worker_observation::{self, Slot, Source},
};
use std::{collections::HashSet, time::Duration};
use thiserror::Error;

/// 当前原型的 Node 角色总数上限, 不表示执行容量
pub const MAX_ROLES: usize = 64;

/// 应用指定的 Authority 身份及有序 Worker 实例列表
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Authority {
    /// 稳定角色身份
    pub id: AuthorityId,
    /// 本次运行身份, 重启时由调用方更换
    pub instance: AuthorityInstanceId,
    /// 静态轮转顺序, 可包含本地及远端 Worker
    pub workers: Vec<WorkerInstanceId>,
}

/// 应用指定的 Worker 身份及唯一 Authority 绑定
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Worker {
    /// 稳定角色身份
    pub id: WorkerId,
    /// 本次运行身份, 重启时由调用方更换
    pub instance: WorkerInstanceId,
    /// 授权方实例, 可位于本 Node 或远端
    pub authority: AuthorityInstanceId,
}

/// 与存储, 网络, 编码及观测出口分离的角色配置
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    /// Node 稳定身份, 库不生成运行实例 ID
    pub node_id: NodeId,
    /// 宿主驱动使用的共同排空期限
    pub drain_timeout: Duration,
    /// 本 Node 承载的 Authority, 按应用配置顺序保存
    pub authorities: Vec<Authority>,
    /// 本 Node 承载的 Worker, 按应用配置顺序保存
    pub workers: Vec<Worker>,
}

impl Config {
    /// 原子校验角色身份和双向绑定, 生成不可变的组装计划
    ///
    /// 允许空角色, 仅 Authority, 仅 Worker 或混合配置. 不要求网络地址,
    /// 不验证远端身份所有权, 路由可达性或跨重启实例是否已使用.
    /// # 错误
    /// 期限, 角色数量, 重复身份, 错角色引用或本地绑定不一致时拒绝
    pub fn into_plan(self) -> Result<Plan, Error> {
        if !crate::lifecycle::valid_drain_timeout(self.drain_timeout) {
            return Err(Error::InvalidDrainTimeout);
        }
        if self.authorities.len().saturating_add(self.workers.len()) > MAX_ROLES {
            return Err(Error::TooManyRoles);
        }
        let mut identities = HashSet::new();
        for identity in self
            .authorities
            .iter()
            .map(|role| role.id.as_u128())
            .chain(self.workers.iter().map(|role| role.id.as_u128()))
        {
            if !identities.insert(identity) {
                return Err(Error::DuplicateIdentity(identity));
            }
        }
        let mut instances = HashSet::new();
        for instance in self
            .authorities
            .iter()
            .map(|role| RoleInstanceId::from(role.instance))
            .chain(
                self.workers
                    .iter()
                    .map(|role| RoleInstanceId::from(role.instance)),
            )
        {
            if !instances.insert(instance) {
                return Err(Error::DuplicateInstance(instance));
            }
        }
        for authority in &self.authorities {
            if authority.workers.is_empty() {
                return Err(Error::NoWorkers(authority.instance));
            }
            if authority.workers.len() > worker_observation::MAX_WORKERS {
                return Err(Error::TooManyWorkers(authority.instance));
            }
            let mut workers = HashSet::new();
            for worker in &authority.workers {
                if !workers.insert(*worker) {
                    return Err(Error::DuplicateWorker {
                        authority: authority.instance,
                        worker: *worker,
                    });
                }
                if self
                    .authorities
                    .iter()
                    .any(|other| other.instance.as_u128() == worker.as_u128())
                {
                    return Err(Error::WorkerTargetsAuthority(*worker));
                }
                if let Some(local) = self.workers.iter().find(|local| local.instance == *worker)
                    && local.authority != authority.instance
                {
                    return Err(Error::WorkerBoundElsewhere {
                        authority: authority.instance,
                        worker: *worker,
                    });
                }
            }
        }
        for worker in &self.workers {
            if self
                .workers
                .iter()
                .any(|other| other.instance.as_u128() == worker.authority.as_u128())
            {
                return Err(Error::AuthorityTargetsWorker(worker.authority));
            }
            if let Some(local) = self
                .authorities
                .iter()
                .find(|local| local.instance == worker.authority)
                && !local.workers.contains(&worker.instance)
            {
                return Err(Error::WorkerNotListed {
                    authority: worker.authority,
                    worker: worker.instance,
                });
            }
        }
        Ok(Plan { config: self })
    }
}

/// 已校验的不可变计划; 应用显式消费分组来构造并启动角色
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Plan {
    config: Config,
}

impl Plan {
    /// Node 稳定身份, 不生成随机数
    pub fn node_id(&self) -> NodeId {
        self.config.node_id
    }
    /// 已校验的共同排空期限
    pub fn drain_timeout(&self) -> Duration {
        self.config.drain_timeout
    }
    /// 保留应用给出的 Authority 及轮转顺序
    pub fn authorities(&self) -> &[Authority] {
        &self.config.authorities
    }
    /// 本 Node 全部 Worker, 包含本地组合及绑定远端 Authority 的 Worker
    pub fn workers(&self) -> &[Worker] {
        &self.config.workers
    }
    /// 按 Authority 分组本地 Worker, 未承载的 Authority 不产生本地组合
    pub fn local_workers(&self, authority: AuthorityInstanceId) -> impl Iterator<Item = &Worker> {
        let local = self
            .config
            .authorities
            .iter()
            .any(|role| role.instance == authority);
        self.config
            .workers
            .iter()
            .filter(move |worker| local && worker.authority == authority)
    }
    /// 必须独立驱动并向远端 Authority 交付的本地 Worker
    pub fn remote_workers(&self) -> impl Iterator<Item = &Worker> {
        self.config.workers.iter().filter(|worker| {
            !self
                .config
                .authorities
                .iter()
                .any(|role| role.instance == worker.authority)
        })
    }
    /// 按原 Worker 顺序投影观察槽位; 未承载的 Authority 返回空迭代
    pub fn worker_slots(&self, authority: AuthorityInstanceId) -> impl Iterator<Item = Slot> {
        self.config
            .authorities
            .iter()
            .find(|role| role.instance == authority)
            .into_iter()
            .flat_map(|role| &role.workers)
            .map(|instance| Slot {
                instance: *instance,
                source: if self
                    .config
                    .workers
                    .iter()
                    .any(|worker| worker.instance == *instance)
                {
                    Source::Local
                } else {
                    Source::Remote
                },
            })
    }
}

/// 组装配置错误, 不涉及网络或执行状态
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum Error {
    /// 排空期限超出原型允许范围
    #[error("Draining 期限必须为 1..=60000 毫秒")]
    InvalidDrainTimeout,
    /// 本 Node 承载角色过多
    #[error("每个 Node 最多配置 64 个角色实例")]
    TooManyRoles,
    /// 不同本地角色复用同一稳定身份值
    #[error("逻辑角色身份重复: {0:032x}")]
    DuplicateIdentity(u128),
    /// 不同本地角色复用同一运行身份值
    #[error("角色实例身份重复: {0}")]
    DuplicateInstance(RoleInstanceId),
    /// Authority 未指定执行者
    #[error("Authority 至少需要一个 Worker: {0}")]
    NoWorkers(AuthorityInstanceId),
    /// 静态允许列表超过观察表上限
    #[error("每个 Authority 最多配置 64 个 Worker 实例: {0}")]
    TooManyWorkers(AuthorityInstanceId),
    /// 同一 Authority 的列表含重复 Worker
    #[error("Authority {authority} 重复配置 Worker {worker}")]
    DuplicateWorker {
        /// 列表所属 Authority
        authority: AuthorityInstanceId,
        /// 重复 Worker
        worker: WorkerInstanceId,
    },
    /// Worker 列表错误地引用了已知 Authority
    #[error("Worker 目标指向了 Authority 实例: {0}")]
    WorkerTargetsAuthority(WorkerInstanceId),
    /// Authority 引用指向已知 Worker, 包含自我引用
    #[error("Authority 目标指向了 Worker 实例: {0}")]
    AuthorityTargetsWorker(AuthorityInstanceId),
    /// Authority 所列的本地 Worker 绑定了其他 Authority
    #[error("Authority {authority} 的本地 Worker {worker} 绑定了其他 Authority")]
    WorkerBoundElsewhere {
        /// 列表所属 Authority
        authority: AuthorityInstanceId,
        /// 反向绑定不符的 Worker
        worker: WorkerInstanceId,
    },
    /// Worker 所绑的本地 Authority 没有列出它
    #[error("本地 Authority {authority} 未配置 Worker {worker}")]
    WorkerNotListed {
        /// Worker 期望的 Authority
        authority: AuthorityInstanceId,
        /// 不在列表中的 Worker
        worker: WorkerInstanceId,
    },
}
