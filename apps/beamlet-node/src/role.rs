//! 将一种角色的配置和基础设施装配为独立运行实例.
use crate::{
    infrastructure::Infrastructure,
    settings::{Role, RoleSettings},
};
use anyhow::{Context, Result};
use beamlet_api::{AuthorityInfo, RoleInstance};
use beamlet_host::Shutdown;
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
    /// - `infrastructures`: 该角色引用的已创建基础设施, 顺序固定.
    /// - `shutdown`: 停止通知或 future; 完成后停止新接收并按角色语义排空.
    ///
    /// # 返回值
    ///
    /// 正常停止返回 `Ok(())`; 绑定, 构造, 运行或必要的刷新失败时返回带角色上下文的错误.
    pub(crate) async fn run(
        self,
        infrastructures: Vec<Infrastructure>,
        shutdown: Shutdown,
    ) -> Result<()> {
        let Self { settings, instance } = self;
        let span = info_span!("role", role_id = %instance.role_id, role_session_id = %instance.session_id,
            kind = settings.kind.name(), label = settings.label.as_deref(), infrastructures = ?settings.infrastructures);
        async move {
            info!(event = "role.starting", "角色启动");
            let result = match settings.kind {
                Role::Authority(config) => {
                    let info = AuthorityInfo {
                        instance,
                        label: settings.label,
                        pools: config.pools.clone(),
                    };
                    let mut transports = beamlet_authority::Transports::new();
                    for infrastructure in &infrastructures {
                        infrastructure.attach(&info, &mut transports).await?;
                    }
                    let authority = beamlet_authority::Authority::new(info, (&config).into())?;
                    authority.run(transports, shutdown.wait()).await?;
                    for infrastructure in &infrastructures {
                        infrastructure.flush().await?;
                    }
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
                    infrastructures
                        .first()
                        .context("Worker 缺少基础设施")?
                        .run_worker(worker, shutdown)
                        .await
                }
            };
            result.with_context(|| format!("角色 {} 运行失败", instance.role_id))?;
            info!(event = "role.stopped", "角色已退出");
            Ok(())
        }
        .instrument(span)
        .await
    }
}
