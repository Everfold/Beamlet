use beamlet_api::{Call, CapabilityId, Pool, RoleInstance};
use std::{
    collections::{BTreeMap, BTreeSet, btree_map::Entry},
    error::Error,
    fmt,
    future::Future,
    pin::Pin,
    sync::Arc,
};

/// 业务处理器可读取的调用上下文, 不授予创建或替换 Attempt 的权限.
#[derive(Debug, Clone)]
pub struct Context {
    pool: Pool,
    authority: RoleInstance,
    worker: RoleInstance,
    operation_id: String,
    attempt_id: String,
    capability: String,
}
impl Context {
    pub(crate) fn from_call<P>(call: &Call<P>) -> Self {
        Self {
            pool: call.pool.clone(),
            authority: call.authority,
            worker: call.worker,
            operation_id: call.attempt.operation_id().clone(),
            attempt_id: call.attempt.id().clone(),
            capability: call.capability.clone(),
        }
    }
    /// 调用所在的 pool.
    pub fn pool(&self) -> &Pool {
        &self.pool
    }
    /// 签发调用的 Authority 实例.
    pub fn authority(&self) -> RoleInstance {
        self.authority
    }
    /// 执行调用的 Worker 实例.
    pub fn worker(&self) -> RoleInstance {
        self.worker
    }
    /// 原始 Operation 身份.
    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }
    /// 已签发的 Attempt 身份.
    pub fn attempt_id(&self) -> &str {
        &self.attempt_id
    }
    /// 此次执行的能力标识.
    pub fn capability(&self) -> &str {
        &self.capability
    }
}

/// 执行未能产生业务结果, 不会因此伪造 Report 或自动重试.
#[derive(Debug)]
pub struct ExecutionError(Box<dyn Error + Send + Sync>);
impl ExecutionError {
    /// 包装处理器的失败原因; 业务失败若需报告, 可把 Result 用作业务结果类型.
    pub fn new(source: impl Into<Box<dyn Error + Send + Sync>>) -> Self {
        Self(source.into())
    }
}
impl fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl Error for ExecutionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.0.as_ref())
    }
}

/// 注册或启用能力失败; 失败不会覆盖已经注册的处理器.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CapabilityError {
    /// 标识不符合共享能力标识规则.
    #[error("invalid capability {id}: {source}")]
    Invalid {
        /// 未通过校验的标识.
        id: String,
        /// 校验原因.
        #[source]
        source: beamlet_api::InvalidCapabilityId,
    },
    /// 相同标识已经注册.
    #[error("capability already registered: {0}")]
    Duplicate(String),
    /// 所选能力没有处理器.
    #[error("unknown capability: {0}")]
    Unknown(String),
    /// Worker 没有启用能力.
    #[error("at least one capability must be enabled")]
    Empty,
}

type Task<R> = Pin<Box<dyn Future<Output = Result<R, ExecutionError>> + Send>>;
pub(crate) type Handler<P, R> = Arc<dyn Fn(P, Context) -> Task<R> + Send + Sync>;

/// 应用提供的能力集合, 一个标识对应一个异步处理器.
pub struct Capabilities<P = String, R = String> {
    pub(crate) handlers: BTreeMap<String, Handler<P, R>>,
}
impl<P, R> Default for Capabilities<P, R> {
    fn default() -> Self {
        Self {
            handlers: BTreeMap::new(),
        }
    }
}
impl<P, R> fmt::Debug for Capabilities<P, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Capabilities")
            .field("ids", &self.handlers.keys())
            .finish()
    }
}
impl<P, R> Capabilities<P, R> {
    /// 创建空注册表; Worker 构造时要求至少启用一种能力.
    pub fn new() -> Self {
        Self::default()
    }
    /// 注册能力; 处理器自行安排异步 I/O 或阻塞计算.
    ///
    /// # Errors
    /// 标识无效或重复时返回错误, 原注册项保持不变.
    pub fn register<F>(
        &mut self,
        id: impl Into<String>,
        handler: impl Fn(P, Context) -> F + Send + Sync + 'static,
    ) -> Result<(), CapabilityError>
    where
        F: Future<Output = Result<R, ExecutionError>> + Send + 'static,
    {
        let id = id.into();
        CapabilityId::validate(&id).map_err(|source| CapabilityError::Invalid {
            id: id.clone(),
            source,
        })?;
        match self.handlers.entry(id) {
            Entry::Occupied(entry) => Err(CapabilityError::Duplicate(entry.key().clone())),
            Entry::Vacant(entry) => {
                entry.insert(Arc::new(move |payload, context| {
                    Box::pin(handler(payload, context))
                }));
                Ok(())
            }
        }
    }
    /// 枚举注册标识, 按字典顺序返回.
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.handlers.keys().map(String::as_str)
    }
    /// 检查配置选择, 重复名称按集合语义处理.
    ///
    /// # Errors
    /// 集合为空或包含未知能力时返回错误.
    pub fn validate_enabled<'a>(
        &self,
        enabled: impl IntoIterator<Item = &'a str>,
    ) -> Result<(), CapabilityError> {
        let mut empty = true;
        for id in enabled {
            empty = false;
            if !self.handlers.contains_key(id) {
                return Err(CapabilityError::Unknown(id.into()));
            }
        }
        if empty {
            return Err(CapabilityError::Empty);
        }
        Ok(())
    }
    pub(crate) fn retain_enabled(
        &mut self,
        enabled: &BTreeSet<String>,
    ) -> Result<(), CapabilityError> {
        self.validate_enabled(enabled.iter().map(String::as_str))?;
        self.handlers.retain(|id, _| enabled.contains(id));
        Ok(())
    }
}
