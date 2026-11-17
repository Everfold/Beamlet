//! 所有路由均包含 pool, 实例路由同时包含角色和会话身份.
use beamlet_api::{Pool, RoleInstance};

/// 查询一个 pool 中的 Authority, 收集所有回复.
pub fn discovery(pool: &Pool) -> String {
    format!("beamlet.v3.{pool}.authority.discover")
}
/// 向一个 Authority 实例发送控制或创建请求.
pub fn authority(pool: &Pool, instance: RoleInstance) -> String {
    format!(
        "beamlet.v3.{pool}.authority.{}.{}.requests",
        instance.role_id, instance.session_id
    )
}
/// 一个 pool 的 Worker 状态广播.
pub fn presence(pool: &Pool) -> String {
    format!("beamlet.v3.{pool}.worker.presence")
}
/// 请求 Worker 立即发布新快照.
pub fn probe(pool: &Pool) -> String {
    format!("beamlet.v3.{pool}.worker.probe")
}
/// 向唯一 Worker 实例交付调用.
pub fn worker(pool: &Pool, instance: RoleInstance) -> String {
    format!(
        "beamlet.v3.{pool}.worker.{}.{}.calls",
        instance.role_id, instance.session_id
    )
}
