//! 将一种角色的配置和基础设施装配为独立运行实例.
use crate::{
    local,
    settings::{Role, RoleSettings},
    terminal,
};
use anyhow::{Context, Result};
use beamlet_api::{AuthorityInfo, RoleInstance};
use beamlet_host::Shutdown;
use beamlet_transport_nats::Nats;
use tokio::sync::mpsc;
use tracing::{Instrument, info, info_span};
use uuid::Uuid;

/// 单个角色的装配信息, 包含持久配置和新生成的本次会话身份.
pub(crate) struct RunningRole {
    /// 该角色的持久配置及命名基础设施引用.
    pub settings: RoleSettings,
    /// 由固定 role_id 和本次新 session_id 组成的运行身份.
    pub instance: RoleInstance,
}
impl RunningRole {
    /// 使用角色固定身份并生成本次运行的会话 UUID.
    ///
    /// # 参数
    ///
    /// - `settings`: 单个角色的固定 UUID, 标签, 基础设施引用和角色专属配置.
    ///
    /// # 返回值
    ///
    /// 携带原角色配置和新会话身份的角色运行实例.
    pub(crate) fn new(settings: RoleSettings) -> Self {
        let instance = RoleInstance {
            role_id: settings.role_id,
            session_id: Uuid::new_v4(),
        };
        Self { settings, instance }
    }
    /// 根据角色种类装配并运行 Authority 或 Worker.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `RunningRole` 的值; 调用后转移其所有权(Copy 类型按值复制).
    /// - `infrastructure`: 该角色使用的已连接 NATS 句柄, 可与其他角色共享.
    /// - `local`: 分配给该角色的本地终端队列; Worker 不使用文本提交.
    /// - `shutdown`: 停止通知或 future; 完成后停止新接收并按角色语义排空.
    ///
    /// # 返回值
    ///
    /// 正常停止返回 `Ok(())`; 绑定, 构造, 运行或必要的刷新失败时返回带角色上下文的错误.
    pub(crate) async fn run(
        self,
        infrastructure: Nats,
        local: terminal::Input,
        shutdown: Shutdown,
    ) -> Result<()> {
        let Self { settings, instance } = self;
        let span = info_span!("role", role_id = %instance.role_id, role_session_id = %instance.session_id,
            kind = settings.kind.name(), label = settings.label.as_deref(), infrastructure = %settings.infrastructure);
        async move {
            info!(event = "role.starting", "角色启动");
            let result = match settings.kind {
                Role::Authority(config) => {
                    let info = AuthorityInfo {
                        instance,
                        label: settings.label,
                        pools: config.pools.clone(),
                    };
                    let endpoint = infrastructure.bind_authority(&info).await?;
                    let authority =
                        beamlet_authority::Authority::new(info.clone(), (&config).into())?;
                    let input = local::Input::new(info, authority.directory(), local)?;
                    authority
                        .run_with_local((endpoint, infrastructure.clone()), input, shutdown.wait())
                        .await?;
                    infrastructure.flush().await?;
                    Ok(())
                }
                Role::Worker(config) => {
                    let mut builder = beamlet_worker::Worker::builder(
                        instance,
                        crate::capabilities::registered()?,
                    )
                    .pool(config.pool.clone())
                    .options((&config).into())
                    .enabled(config.capabilities);
                    if let Some(label) = settings.label {
                        builder = builder.label(label);
                    }
                    let worker = builder.build()?;
                    let (inbox, presence) = infrastructure.bind_worker(worker.binding()).await?;
                    worker
                        .run(inbox, presence, shutdown.wait())
                        .await
                        .map_err(anyhow::Error::from)
                }
            };
            result.with_context(|| format!("角色 {} 运行失败", instance.role_id))?;
            info!(event = "role.stopped", "角色已退出");
            Ok(())
        }
        .instrument(span)
        .await
    }
    /// 按角色配置创建本地队列, Authority 同时提供终端路由目标.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `RunningRole` 的共享引用.
    ///
    /// # 返回值
    ///
    /// 二元组 `(目标, 接收端)`; Authority 的目标为 `Some`, Worker 的目标为 `None`.
    pub(crate) fn local_input(&self) -> (Option<terminal::Target>, terminal::Input) {
        let (send, receive) = mpsc::channel(match &self.settings.kind {
            Role::Authority(config) => config.input_capacity,
            Role::Worker(_) => 1,
        });
        let target = matches!(self.settings.kind, Role::Authority(_)).then(|| terminal::Target {
            instance: self.instance,
            label: self.settings.label.clone(),
            send,
        });
        (target, receive)
    }
}
