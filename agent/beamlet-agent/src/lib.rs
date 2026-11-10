//! Agent 层的预留入口与原型支持, 当前保留通用执行库的兼容导出

/// 通用执行角色的兼容导出, 独立使用时可直接依赖 beamlet
pub use beamlet::{Authority, Worker};

/// 原本地组合入口的兼容名称, 实现位于 [`beamlet::LocalExecution`]
pub use beamlet::LocalExecution as Agent;
/// 原本地组合错误的兼容名称, 实现位于 [`beamlet::LocalExecutionError`]
pub use beamlet::LocalExecutionError as AgentError;

/// 显式启用的固定假任务与 v0 JSON 测试支持, 不属于通用 Agent 契约
#[cfg(feature = "test-support")]
pub mod testing;

/// 旧调用方使用的通用执行类型, 新代码可直接依赖 beamlet
pub use beamlet::{
    executor::ExecutorError,
    id,
    memory::{
        Admission, Limits, OperationPhase, OperationView, ReportDisposition, StateError,
        Submission, WaitStatus,
    },
    model, state,
};
