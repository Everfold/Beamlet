//! 并发在线调用演示: 启动器 -> Authority -> Worker, 三者运行在不同进程.
//!
//! Authority 管理生命周期, Worker 执行业务函数. 两个角色通过匿名管道交换
//! 以换行分隔的 JSON 请求和报告, 复用同一个 Worker 完成两次调用.
//! 不需要监听端口, 消息代理或数据库.

mod protocol;

use anyhow::{Context, Result, bail, ensure};
use beamlet_primitives::{Attempt, Report};
use beamlet_runtime::Runtime;
use protocol::{Call, Function, Reply};
use std::{env, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
    task::JoinSet,
    time::{sleep, timeout},
};

/// 启动演示或进入指定角色, 整次调用限定在十秒内完成.
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // 每个角色独立设置超时; Tokio current_thread 只限定执行器线程数, 不替代子进程隔离.
    timeout(Duration::from_secs(10), run())
        .await
        .context("演示超过十秒期限")?
}

/// 默认启动 Authority 子进程, Authority 再启动独立的 Worker 子进程.
async fn run() -> Result<()> {
    let args: Vec<_> = env::args().skip(1).collect();
    match args.as_slice() {
        [] => {
            // 复用当前可执行文件, 用参数选择角色; 这里启动真实进程, 而非 Tokio task.
            // stdout/stderr 继承到终端, 让 Authority 的结果与错误可见.
            let mut authority = Command::new(env::current_exe()?)
                .arg("--authority")
                .stdin(Stdio::null())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .kill_on_drop(true)
                .spawn()
                .context("启动管理进程失败")?;
            ensure!(authority.wait().await?.success(), "管理进程执行失败");
            Ok(())
        }
        [role] if role == "--authority" => authority().await,
        [role] if role == "--worker" => worker().await,
        _ => bail!("用法: beamlet-node [--authority | --worker]"),
    }
}

/// Authority 连续发送不同请求, 按报告身份处理乱序返回并关闭对应 Operation.
async fn authority() -> Result<()> {
    let mut runtime = Runtime::new();
    // stdin 承载请求, stdout 承载报告; stderr 单独继承, 避免日志混入 JSON.
    // kill_on_drop 用于出错或超时后终止直接子进程.
    let mut worker = Command::new(env::current_exe()?)
        .arg("--worker")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .context("启动工作进程失败")?;
    let worker_pid = worker.id().context("获取进程号前工作进程已退出")?;
    let mut input = worker.stdin.take().context("缺少工作进程输入管道")?;
    // 缓冲读取器跨调用复用, 不丢弃提前读入的后续消息.
    let mut replies = BufReader::new(worker.stdout.take().context("缺少工作进程输出管道")?).lines();
    println!("[管理进程] 进程号={}", std::process::id());
    println!("[管理进程] 工作进程号={worker_pid}");

    // 第一个请求延迟完成, 第二个请求使用不同函数和参数.
    let requests = [
        (1_u64, 101_u64, Function::HelloWorld, 300),
        (
            2,
            102,
            Function::Uppercase {
                text: "hello beamlet".into(),
            },
            0,
        ),
    ];
    for (operation_id, attempt_id, request, delay_ms) in requests {
        runtime.create_operation(operation_id, request)?;
        let attempt = runtime.create_attempt(&operation_id, attempt_id)?;
        let request = runtime
            .operation(&operation_id)
            .context("找不到操作")?
            .request()
            .clone();
        println!(
            "[管理进程] 发送调用: 操作={operation_id}, 授权={attempt_id}, 请求={request:?}, 延迟={delay_ms}毫秒, 状态=等待报告."
        );
        let call = Call {
            operation_id: *attempt.operation_id(),
            attempt_id: *attempt.id(),
            request,
            delay_ms,
        };
        let mut bytes = serde_json::to_vec(&call)?;
        bytes.push(b'\n');
        input
            .write_all(&bytes)
            .await
            .context("向工作进程发送调用失败")?;
    }
    // 不等待报告就发送下一份请求. EOF 只结束接收, Worker 仍需完成已接收任务.
    input.shutdown().await?;
    drop(input);
    println!("[管理进程] 两份请求已发送, 按实际完成顺序接收报告.");

    for _ in 0..2 {
        let line = replies
            .next_line()
            .await
            .context("读取工作进程报告失败")?
            .context("工作进程在报告前关闭了输出管道")?;
        let reply: Reply = serde_json::from_str(&line).context("解析工作进程报告失败")?;
        ensure!(reply.worker_pid == worker_pid, "报告的工作进程号不匹配");
        let report = Report::new(
            reply.report_id,
            Attempt::from_parts(reply.operation_id, reply.attempt_id),
            reply.result,
        );
        // 按报告中的身份路由, 不按请求发送顺序取出 Operation.
        runtime.accept_report(&report)?;
        let operation_id = *report.operation_id();
        let expected = match operation_id {
            1 => "Hello World",
            2 => "HELLO BEAMLET",
            _ => bail!("报告引用了未知操作"),
        };
        ensure!(report.result() == expected, "函数返回值不符合对应请求");
        runtime.close(&operation_id)?;
        println!(
            "[管理进程] 调用完成: 操作={}, 授权={}, 报告={}, 工作进程号={}, 返回值={}, 状态=已关闭.",
            operation_id,
            report.attempt_id(),
            report.id(),
            worker_pid,
            report.result(),
        );
    }

    ensure!(
        replies.next_line().await?.is_none(),
        "收到多余的工作进程报告"
    );
    ensure!(worker.wait().await?.success(), "工作进程执行失败");
    println!("[管理进程] 工作进程已正常退出.");
    Ok(())
}

/// Worker 同时接收请求和收集任务结果, EOF 后排空任务再退出.
async fn worker() -> Result<()> {
    worker_log(format!(
        "[工作进程] 已启动, 进程号={}, 等待调用.",
        std::process::id()
    ))?;
    let mut calls = BufReader::new(tokio::io::stdin()).lines();
    let mut output = tokio::io::stdout();
    let mut tasks = JoinSet::new();
    let mut input_closed = false;
    let mut report_id = 0_u64;
    while !input_closed || !tasks.is_empty() {
        tokio::select! {
            // 限制在途任务数, 满载时暂停读取, 让管道提供背压.
            line = calls.next_line(), if !input_closed && tasks.len() < 16 => {
                match line? {
                    Some(line) => {
                        let call: Call = serde_json::from_str(&line).context("解析调用失败")?;
                        report_id = report_id.checked_add(1).context("报告身份已耗尽")?;
                        worker_log(format!("[工作进程] 收到调用: 操作={}, 授权={}, 请求={:?}.",
                            call.operation_id, call.attempt_id, call.request))?;
                        tasks.spawn(execute_call(call, report_id));
                    }
                    None => {
                        input_closed = true;
                        worker_log("[工作进程] 请求管道已关闭, 等待在途任务完成.".to_owned())?;
                    }
                }
            }
            completed = tasks.join_next(), if !tasks.is_empty() => {
                let reply = completed.context("缺少任务结果")?.context("工作任务异常退出")?;
                let mut bytes = serde_json::to_vec(&reply)?;
                bytes.push(b'\n');
                // 只有此循环写 stdout, 避免并发任务的 JSON 片段交错.
                output.write_all(&bytes).await?;
                output.flush().await?;
                worker_log(format!("[工作进程] 已发送报告={}, 操作={}, 返回值={:?}.",
                    reply.report_id, reply.operation_id, reply.result))?;
            }
        }
    }
    worker_log("[工作进程] 全部任务完成, 正常退出.".to_owned())?;
    Ok(())
}

/// 异步执行一次请求, 仅返回报告数据, 不直接写通信管道.
async fn execute_call(call: Call, report_id: u64) -> Reply {
    // 延迟只用于演示乱序, 不阻塞接收其他调用.
    sleep(Duration::from_millis(call.delay_ms)).await;
    let result = match call.request {
        Function::HelloWorld => hello_world().to_owned(),
        Function::Uppercase { text } => uppercase(&text),
    };
    let report = Report::new(
        report_id,
        Attempt::from_parts(call.operation_id, call.attempt_id),
        result,
    );
    Reply {
        operation_id: *report.operation_id(),
        attempt_id: *report.attempt_id(),
        report_id: *report.id(),
        worker_pid: std::process::id(),
        result: report.into_result(),
    }
}

/// 将请求文本转换为 Unicode 大写, 保留换行等其他字符.
fn uppercase(text: &str) -> String {
    text.to_uppercase()
}

/// 先拼好整行再写入 stderr, 减少跨进程日志片段交错, 不污染报告管道.
fn worker_log(message: String) -> std::io::Result<()> {
    use std::io::Write;
    std::io::stderr()
        .lock()
        .write_all(format!("{message}\n").as_bytes())
}

/// 被 Authority 远程调用的最小业务函数.
fn hello_world() -> &'static str {
    "Hello World"
}
