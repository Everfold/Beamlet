//! 所有路由均包含 pool, 实例路由同时包含角色和会话身份.
use beamlet_api::{Pool, RoleInstance};

/// 构造一个 pool 的 Authority 发现主题字符串.
///
/// # 参数
///
/// - `pool`: 查询, 候选选择或主题构造的目标 pool.
///
/// # 返回值
///
/// `beamlet.v3.<pool>.authority.discover`; 不发送消息.
pub fn discovery(pool: &Pool) -> String {
    format!("beamlet.v3.{pool}.authority.discover")
}
/// 构造指向唯一 Authority 角色会话的请求主题.
///
/// # 参数
///
/// - `pool`: 查询, 候选选择或主题构造的目标 pool.
/// - `instance`: 当前角色的固定 UUID 和本次运行会话 UUID; 不会在此重新生成.
///
/// # 返回值
///
/// 含 pool, 角色 UUID 与会话 UUID 的 Authority 请求主题字符串.
pub fn authority(pool: &Pool, instance: RoleInstance) -> String {
    format!(
        "beamlet.v3.{pool}.authority.{}.{}.requests",
        instance.role_id, instance.session_id
    )
}
/// 构造一个 pool 的 Worker 状态公告主题.
///
/// # 参数
///
/// - `pool`: 查询, 候选选择或主题构造的目标 pool.
///
/// # 返回值
///
/// `beamlet.v3.<pool>.worker.presence`; 不发布公告.
pub fn presence(pool: &Pool) -> String {
    format!("beamlet.v3.{pool}.worker.presence")
}
/// 构造一个 pool 的 Worker 探测主题.
///
/// # 参数
///
/// - `pool`: 查询, 候选选择或主题构造的目标 pool.
///
/// # 返回值
///
/// `beamlet.v3.<pool>.worker.probe`; 不发起探测.
pub fn probe(pool: &Pool) -> String {
    format!("beamlet.v3.{pool}.worker.probe")
}
/// 构造指向唯一 Worker 角色会话的调用主题.
///
/// # 参数
///
/// - `pool`: 查询, 候选选择或主题构造的目标 pool.
/// - `instance`: 当前角色的固定 UUID 和本次运行会话 UUID; 不会在此重新生成.
///
/// # 返回值
///
/// 含 pool, 角色 UUID 与会话 UUID 的 Worker 调用主题字符串.
pub fn worker(pool: &Pool, instance: RoleInstance) -> String {
    format!(
        "beamlet.v3.{pool}.worker.{}.{}.calls",
        instance.role_id, instance.session_id
    )
}
