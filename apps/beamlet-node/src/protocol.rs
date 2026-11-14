//! 本地管道使用的 JSON 数据格式, 与 kernel 原语的内存表示解耦.
//!
//! Call 传递请求和授权引用, Reply 传回完整报告字段. 示例使用 u64 身份,
//! 不代表框架限定身份类型. deny_unknown_fields 可及时暴露字段拼写或协议不匹配.

use serde::{Deserialize, Serialize};

/// 示例允许调用的函数, 未知名称在解码时拒绝.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "function",
    content = "arguments",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Function {
    /// 在线路上编码为 hello_world, 对应 Worker 的同名业务函数.
    HelloWorld,
    /// 接收文本参数, 返回转换为大写的文本.
    Uppercase { text: String },
}

/// 一次在线调用的传输格式, 不增加领域原语.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Call {
    /// 本次授权所属的 Operation, 供报告原样关联.
    pub operation_id: u64,
    /// Authority 签发的执行授权身份, Worker 不生成新的 Attempt 身份.
    pub attempt_id: u64,
    /// 函数与其参数一同编码, 避免将参数与错误的函数组合.
    pub request: Function,
    /// 演示用执行延迟, 让并发任务的完成顺序可见, 默认不延迟.
    #[serde(default)]
    pub delay_ms: u64,
}

/// 完整报告的传输格式, PID 仅用于演示进程隔离.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    /// 用于 Authority 查找对应的运行中 Operation.
    pub operation_id: u64,
    /// 用于 kernel 检查是否为当前等待的授权.
    pub attempt_id: u64,
    /// 本份报告的身份, 与 Operation 和 Attempt 身份属于不同概念.
    pub report_id: u64,
    /// 证明演示走过独立进程的辅助信息, 不参与领域状态转换.
    pub worker_pid: u32,
    /// 对应业务函数的返回值, 可以包含换行或非 ASCII 文本.
    pub result: String,
}
