//! 由通用本地执行组合驱动的单进程演示, 应用不自行拼装执行循环

use super::{
    executor::execute,
    protocol::{Limits, TestTask, Uuid, Value},
};
use beamlet::{
    Authority, LocalClientError, LocalExecution, LocalExecutionError, ReportObservation, Worker,
    id::{RoleInstanceId, SubmissionKey},
    model::{ExecutionResult, Message},
};
use thiserror::Error;

/// 一次完整演示的可观察结果, 不把预期失败算成驱动错误
#[derive(Debug)]
pub struct DemoResult {
    /// 已收录报告数
    pub completed: usize,
    /// 成功报告数
    pub succeeded: usize,
    /// 固定 fail 分支的报告数
    pub failed: usize,
    /// 两个静态 Worker 各完成多少请求
    pub worker_counts: [usize; 2],
    /// 按原提交键找回执行后查询到的首份报告
    pub sample: ReportObservation<Value, String>,
}

/// 运行原有 100 请求演示, 使用真实 Tokio 计时器
///
/// # 错误
/// 返回配置, 驱动或结果关联错误, 不自动重跑
pub async fn run() -> Result<DemoResult, DemoError> {
    let limits = Limits::default();
    let authority_id: RoleInstanceId = Uuid::new_v4().as_u128().into();
    let workers: [RoleInstanceId; 2] = [
        Uuid::new_v4().as_u128().into(),
        Uuid::new_v4().as_u128().into(),
    ];
    let authority =
        Authority::<TestTask, Value, String>::new(authority_id, workers.to_vec(), limits.core())
            .map_err(LocalExecutionError::from)?;
    let worker = Worker::new(authority_id, workers.to_vec(), limits.core())
        .map_err(LocalExecutionError::from)?;
    let mut execution = LocalExecution::from_parts(authority, worker);
    let mut first: Option<SubmissionKey> = None;
    let mut allocate = || {
        (
            Uuid::new_v4().as_u128().into(),
            Uuid::new_v4().as_u128().into(),
        )
    };
    {
        let mut client = execution.client(&mut allocate);
        for index in 0..100 {
            let key = Uuid::new_v4().as_u128().into();
            let task = match index % 3 {
                0 => TestTask::Echo(serde_json::json!({"request": index})),
                1 => TestTask::Delay {
                    delay_ms: 10,
                    value: serde_json::json!({"request": index}),
                },
                _ => TestTask::Fail {
                    message: format!("预期测试失败 {index}"),
                },
            };
            client.submit(key, task)?;
            first.get_or_insert(key);
        }
    }
    let mut completed = 0;
    let mut succeeded = 0;
    let mut failed = 0;
    let mut worker_counts = [0; 2];
    while let Some(delivery) = execution.next_report(execute).await? {
        let Message::SubmitReport(report) = &delivery.message else {
            return Err(DemoError::Incomplete);
        };
        match &report.result {
            ExecutionResult::Succeeded { .. } => succeeded += 1,
            ExecutionResult::Failed { .. } => failed += 1,
            ExecutionResult::Rejected { .. } => return Err(DemoError::Incomplete),
        }
        let worker = workers
            .iter()
            .position(|id| *id == delivery.source_instance_id)
            .ok_or(DemoError::Incomplete)?;
        worker_counts[worker] += 1;
        completed += 1;
    }
    if completed != 100 {
        return Err(DemoError::Incomplete);
    }
    let client = execution.client(&mut allocate);
    let recovered = client.query_submission(first.ok_or(DemoError::Incomplete)?)?;
    let view = client.query_operation(recovered.binding.operation_id)?;
    let sample = client.query_report(view.adopted_report.ok_or(DemoError::Incomplete)?)?;
    Ok(DemoResult {
        completed,
        succeeded,
        failed,
        worker_counts,
        sample,
    })
}

/// 两个应用共享的文本演示入口; 业务推进不位于应用 main 中
///
/// # 错误
/// 执行或结果查询失败时返回错误
pub async fn print() -> Result<(), DemoError> {
    let result = run().await?;
    println!("单进程内存演示: 1 个 Authority, 2 个 Worker; 无网络, 无持久化");
    println!(
        "完成 {}/100; 成功 {}, 预期失败 {}",
        result.completed, result.succeeded, result.failed
    );
    println!(
        "两个 Worker 分别完成: {}, {}",
        result.worker_counts[0], result.worker_counts[1]
    );
    println!("按首个提交键找回执行并查询报告:\n{:#?}", result.sample);
    Ok(())
}

/// 演示支持错误, 不属于通用 Agent 的领域契约
#[derive(Debug, Error)]
pub enum DemoError {
    /// 本地执行驱动失败
    #[error(transparent)]
    Execution(#[from] LocalExecutionError),
    /// 原型固定场景未获得完整关联结果
    #[error("演示未保留全部预期结果")]
    Incomplete,
    /// Client 提交或结果查询失败
    #[error(transparent)]
    Client(#[from] LocalClientError),
}
