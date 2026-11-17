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
    /// 从调用中复制业务处理器需要的只读路由与身份上下文。
    ///
    /// # 参数
    ///
    /// - `call`：已指定 pool、Authority 和 Worker 会话的调用，包含 Attempt、能力及业务输入。
    ///
    /// # 返回值
    ///
    /// Context，保留原 pool、角色会话、Operation、Attempt 和能力信息，不持有 payload。
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
    /// 借用调用所属的 pool。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Context` 的共享引用。
    ///
    /// # 返回值
    ///
    /// Context 中保存的 Pool 引用。
    pub fn pool(&self) -> &Pool {
        &self.pool
    }
    /// 读取签发当前调用的 Authority 实例。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Context` 的共享引用。
    ///
    /// # 返回值
    ///
    /// 包含角色与会话 UUID 的 RoleInstance。
    pub fn authority(&self) -> RoleInstance {
        self.authority
    }
    /// 读取执行当前调用的 Worker 实例。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Context` 的共享引用。
    ///
    /// # 返回值
    ///
    /// 包含角色与会话 UUID 的 RoleInstance。
    pub fn worker(&self) -> RoleInstance {
        self.worker
    }
    /// 借用当前调用的 Operation 身份。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Context` 的共享引用。
    ///
    /// # 返回值
    ///
    /// Operation 身份字符串切片。
    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }
    /// 借用当前调用已签发的 Attempt 身份。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Context` 的共享引用。
    ///
    /// # 返回值
    ///
    /// Attempt 身份字符串切片。
    pub fn attempt_id(&self) -> &str {
        &self.attempt_id
    }
    /// 借用本次调用的能力标识。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Context` 的共享引用。
    ///
    /// # 返回值
    ///
    /// 能力名称字符串切片。
    pub fn capability(&self) -> &str {
        &self.capability
    }
}

/// 执行未能产生业务结果, 不会因此伪造 Report 或自动重试.
#[derive(Debug)]
pub struct ExecutionError(Box<dyn Error + Send + Sync>);
impl ExecutionError {
    /// 包装底层失败原因并保留错误链。
    ///
    /// # 参数
    ///
    /// - `source`：底层失败原因；包装后仍可通过错误 source 链访问。
    ///
    /// # 返回值
    ///
    /// 持有原始错误的具体错误对象，不修改或重试失败操作。
    pub fn new(source: impl Into<Box<dyn Error + Send + Sync>>) -> Self {
        Self(source.into())
    }
}
impl fmt::Display for ExecutionError {
    /// 向格式化目标写入 `ExecutionError` 的显示文本。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `ExecutionError` 的共享引用。
    /// - `f`：接收格式化文本的写入目标。
    ///
    /// # 返回值
    ///
    /// 写入成功返回 `Ok(())`；格式化目标拒绝写入时返回 `fmt::Error`。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl Error for ExecutionError {
    /// 借用被包装的底层错误。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `ExecutionError` 的共享引用。
    ///
    /// # 返回值
    ///
    /// 始终为 `Some` 的错误引用，可继续访问其 source 链。
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
    /// 创建尚未注册处理器的空能力集合。
    ///
    /// # 参数
    ///
    /// 无。
    ///
    /// # 返回值
    ///
    /// 空 Capabilities；构造 Worker 前需注册并启用至少一个处理器。
    fn default() -> Self {
        Self {
            handlers: BTreeMap::new(),
        }
    }
}
impl<P, R> fmt::Debug for Capabilities<P, R> {
    /// 向格式化目标写入 `Capabilities<P, R>` 的调试信息。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Capabilities<P, R>` 的共享引用。
    /// - `f`：接收格式化文本的写入目标。
    ///
    /// # 返回值
    ///
    /// 写入成功返回 `Ok(())`；格式化目标拒绝写入时返回 `fmt::Error`。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Capabilities")
            .field("ids", &self.handlers.keys())
            .finish()
    }
}
impl<P, R> Capabilities<P, R> {
    /// 创建尚未注册处理器的空能力集合。
    ///
    /// # 参数
    ///
    /// 无。
    ///
    /// # 返回值
    ///
    /// 空 Capabilities；构造 Worker 前需注册并启用至少一个处理器。
    pub fn new() -> Self {
        Self::default()
    }
    /// 注册一个异步能力处理器，拒绝覆盖同名能力。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Capabilities<P, R>` 的可变引用。
    /// - `id`：要注册的能力名称，必须合法且尚未注册。
    /// - `handler`：接收业务输入和只读 Context 的异步处理器；返回业务结果或 ExecutionError。
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`；标识非法或重复返回 CapabilityError，原注册项不变。
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
    /// 按字典顺序遍历已注册的能力标识。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Capabilities<P, R>` 的共享引用。
    ///
    /// # 返回值
    ///
    /// 借用能力名称的迭代器，不复制处理器。
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.handlers.keys().map(String::as_str)
    }
    /// 校验启用集合非空且所有能力均已注册。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Capabilities<P, R>` 的共享引用。
    /// - `enabled`：需要启用的能力名称；按集合语义处理，不隐式注册未知能力。
    ///
    /// # 返回值
    ///
    /// 合法返回 `Ok(())`；空集合或未知能力返回 CapabilityError。
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
    /// 校验并只保留启用集合中的能力处理器。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Capabilities<P, R>` 的可变引用。
    /// - `enabled`：需要启用的能力名称；按集合语义处理，不隐式注册未知能力。
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`；校验失败返回 CapabilityError 且不删减处理器。
    pub(crate) fn retain_enabled(
        &mut self,
        enabled: &BTreeSet<String>,
    ) -> Result<(), CapabilityError> {
        self.validate_enabled(enabled.iter().map(String::as_str))?;
        self.handlers.retain(|id, _| enabled.contains(id));
        Ok(())
    }
}
