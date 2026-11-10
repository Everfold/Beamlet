//! Agent 层的预留入口与原型支持, 通用执行角色由 beamlet 提供

/// 显式启用的固定假任务与 v0 JSON 测试支持, 不属于通用 Agent 契约
#[cfg(feature = "test-support")]
pub mod testing;
