//! Console 提交, 查询与观察固定 Hello 请求, 显示库返回的执行证据

use std::{env, time::Duration};

use anyhow::{Context, Result, bail, ensure};
use beamlet::{
    ReportObservation,
    id::SubmissionKey,
    model::ExecutionResult,
    observe::{Budget, State},
    query::{Observation, Selection},
    remote::{Binding, Client, ClientError, Outcome},
};
use beamlet_message_zenoh::{ClientTransport, Limits, Transport};
use beamlet_protocol_demo::{hello::Task, submission::HelloClientCodec};
use tokio::{
    io::{self, AsyncBufReadExt, AsyncReadExt, BufReader},
    time::timeout,
};
use uuid::Uuid;

mod notifications;
const NODE_INBOX: &str = "beamlet/v0/node/inbox";
const ROUND_TRIP_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_LINE_BYTES: usize = 64 * 1024;
// 原型观察的证据数量上限, 达到后保留原去重事实并返回错误.
const MAX_OBSERVED_REPORTS: usize = 1024;
type HelloClient<'a> = Client<ClientTransport<'a>, HelloClientCodec>;

#[tokio::main(worker_threads = 2)]
async fn main() -> std::process::ExitCode {
    match run().await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("错误: {error:#}");
            std::process::ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<()> {
    let mut endpoint = String::from("tcp/127.0.0.1:7447");
    let mut authority = None;
    let mut message = None;
    let mut submission_key = None;
    let mut query = None;
    let mut observe = false;
    let mut subscribe = false;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--connect" => endpoint = args.next().context("--connect 需要连接地址")?,
            "--authority" => {
                authority = Some(
                    args.next()
                        .context("--authority 需要 UUID")?
                        .parse::<Uuid>()
                        .context("UUID 格式无效")?,
                )
            }
            "--message" => message = Some(args.next().context("--message 需要任务名称")?),
            "--submission-key" => {
                submission_key = Some(
                    args.next()
                        .context("--submission-key 需要 UUID")?
                        .parse::<Uuid>()
                        .context("SubmissionKey UUID 无效")?,
                );
            }
            "--subscribe" => subscribe = true,
            "--query-submission"
            | "--query-operation"
            | "--query-report"
            | "--observe-operation" => {
                ensure!(query.is_none(), "一次只能选择一种查询");
                observe = arg == "--observe-operation";
                let id = args
                    .next()
                    .with_context(|| format!("{arg} 需要 UUID"))?
                    .parse::<Uuid>()
                    .context("查询 UUID 无效")?
                    .as_u128();
                query = Some(match arg.as_str() {
                    "--query-submission" => Selection::Submission(id.into()),
                    "--query-operation" | "--observe-operation" => Selection::Operation(id.into()),
                    _ => Selection::Report(id.into()),
                });
            }
            "--demo" => return Ok(beamlet_protocol_demo::demo::print().await?),
            "--help" | "-h" => {
                println!(
                    "用法: beamlet-console [--connect tcp/127.0.0.1:7447] [--authority UUID] [--subscribe] [--message hello [--submission-key UUID]] [--demo]\n只读查询: --authority UUID 配合 --query-submission UUID / --query-operation UUID / --query-report UUID\n结果观察: --authority UUID --observe-operation UUID; 合并结果通知并查询补齐, 最多等待 10 秒\n--subscribe 启用只读角色启动/退出通知; 输入结束后继续订阅, Ctrl-C 退出"
                );
                return Ok(());
            }
            _ => bail!("未知参数: {arg}"),
        }
    }

    if query.is_some() {
        ensure!(
            authority.is_some(),
            "只读查询需要 --authority UUID 指定原 Authority 运行实例"
        );
        ensure!(
            message.is_none() && submission_key.is_none(),
            "查询不能与提交参数一起使用"
        );
    }
    if submission_key.is_some() && message.is_none() {
        bail!("--submission-key 必须与 --message hello 一起使用");
    }
    let request = message
        .as_deref()
        .map(|command| hello_request(command, submission_key))
        .transpose()?;

    // 每次启动使用新的路由 ID, 不复用其他 Console 或旧进程的回复地址
    let console_id = Uuid::new_v4().simple().to_string();
    let inbox = format!("beamlet/v0/console/{console_id}/inbox");
    let destination = match (query, authority) {
        (Some(_), Some(authority)) => format!(
            "{}{console_id}",
            beamlet_protocol_demo::query::prefix(authority.as_u128().into())
        ),
        (_, Some(authority)) => format!(
            "beamlet/v0/authority/{}/submit/{console_id}",
            authority.simple()
        ),
        _ => format!("{NODE_INBOX}/{console_id}"),
    };
    let mut transport = timeout(
        ROUND_TRIP_TIMEOUT,
        Transport::connect(&endpoint, &inbox, Limits::default()),
    )
    .await
    .context("连接 Node 超时")?
    .context("连接 Node 失败")?;
    let subscription = if subscribe {
        Some(
            transport
                .subscribe(beamlet_protocol_demo::lifecycle::SUBSCRIPTION)
                .await?,
        )
    } else {
        None
    };
    let result_notices = if observe {
        Some(
            transport
                .subscribe(&beamlet_protocol_demo::query::notice_topic(
                    authority
                        .context("结果观察需要 Authority")?
                        .as_u128()
                        .into(),
                ))
                .await?,
        )
    } else {
        None
    };
    let channel = ClientTransport::bind(&mut transport, &destination).await?;
    if !subscribe {
        timeout(ROUND_TRIP_TIMEOUT, channel.wait_for_receiver())
            .await
            .context("Node 订阅尚未就绪")??;
    }
    let mut client = Client::new(
        channel,
        HelloClientCodec::default(),
        authority.map(|id| id.as_u128().into()),
    );
    println!("Console 已连接: {endpoint} (会话 {console_id})");
    let business = async {
        if let Some(notices) = result_notices {
            let Some(Selection::Operation(operation)) = query else {
                bail!("结果观察需要 Operation")
            };
            let mut state = State::new(
                authority
                    .context("结果观察需要 Authority")?
                    .as_u128()
                    .into(),
                operation,
                MAX_OBSERVED_REPORTS,
            )?;
            if let Some(report) = client
                .observe(&mut state, &notices, Budget::default(), || {
                    Uuid::new_v4().as_u128().into()
                })
                .await?
            {
                println!("Authority: {}", report.authority_instance_id);
                println!("Worker: {}", report.worker_instance_id);
                println!("Operation: {}", report.report.operation_id);
                println!("Attempt: {}", report.report.attempt_id);
                display_report(report)?;
            }
            return Ok(());
        }
        if let Some(selection) = query {
            return query_and_display(&mut client, selection).await;
        }
        if let Some(request) = request {
            return submit_and_display(&mut client, request).await;
        }
        interactive(&mut client).await?;
        if subscribe {
            std::future::pending::<Result<()>>().await
        } else {
            Ok(())
        }
    };
    if let Some(subscription) = subscription {
        println!("Client 已启用生命周期订阅; 不发布 Client 生命周期通知");
        tokio::select! {
            result = business => result,
            result = notifications::receive(subscription) => result,
            signal = tokio::signal::ctrl_c() => { signal?; Ok(()) }
        }
    } else {
        business.await
    }
}

async fn interactive(client: &mut HelloClient<'_>) -> Result<()> {
    println!("输入 hello 后按回车, 每次生成新 Operation; Ctrl-D 结束输入");
    let mut input = BufReader::new(io::stdin());
    loop {
        let mut bytes = Vec::new();
        let count = (&mut input)
            .take((MAX_LINE_BYTES + 1) as u64)
            .read_until(b'\n', &mut bytes)
            .await?;
        if count == 0 {
            return Ok(());
        }
        if bytes.len() > MAX_LINE_BYTES {
            bail!("输入行超过 {MAX_LINE_BYTES} 字节");
        }
        if bytes.last() == Some(&b'\n') {
            bytes.pop();
            if bytes.last() == Some(&b'\r') {
                bytes.pop();
            }
        }
        let command = String::from_utf8(bytes).context("任务名称必须是 UTF-8 文本")?;
        let request = match hello_request(&command, None) {
            Ok(request) => request,
            Err(error) => {
                eprintln!("{error}");
                continue;
            }
        };
        submit_and_display(client, request).await?;
    }
}

fn hello_request(command: &str, submission_key: Option<Uuid>) -> Result<(SubmissionKey, Task)> {
    ensure!(
        command.trim().eq_ignore_ascii_case("hello"),
        "当前仅支持固定任务 hello, 不再原样回显输入"
    );
    Ok((
        submission_key.unwrap_or_else(Uuid::new_v4).as_u128().into(),
        Task::Hello,
    ))
}

async fn submit_and_display(
    client: &mut HelloClient<'_>,
    (key, task): (SubmissionKey, Task),
) -> Result<()> {
    println!("SubmissionKey: {key}");
    let submission = match client.submit(key, &task).await {
        Ok(submission) => submission,
        Err(error) => {
            if let ClientError::TimedOut { progress } | ClientError::Transport { progress, .. } =
                &error
                && let Some(binding) = progress.binding
            {
                display_binding(binding);
            }
            return Err(error.into());
        }
    };
    display_binding(submission.binding);
    match submission.outcome {
        Outcome::Report(observation) => display_report(observation)?,
        Outcome::Pending => println!("状态: 待定; 不创建新授权"),
        Outcome::SubmissionError(error) => bail!("提交返回错误, 不是 Report: {error}"),
        _ => bail!("不支持的结果观察"),
    }
    Ok(())
}

async fn query_and_display(client: &mut HelloClient<'_>, selection: Selection) -> Result<()> {
    let observed = client
        .query(Uuid::new_v4().as_u128().into(), selection)
        .await?;
    match observed {
        Observation::Operation(view) => {
            println!("Authority: {}", view.binding.authority_instance_id);
            println!("Worker: {}", view.binding.worker_instance_id);
            println!("Operation: {}", view.binding.operation_id);
            println!("Attempt: {}", view.binding.attempt_id);
            println!(
                "任务: {:?}; 观察阶段: {:?}; 等待状态: {:?}",
                view.task, view.phase, view.wait_status
            );
            match view.adopted_report {
                Some(id) => println!("采用的 Report: {id}"),
                None => println!("尚无采用的 Report"),
            }
        }
        Observation::Report(view) => {
            println!("Authority: {}", view.authority_instance_id);
            println!("Worker: {}", view.worker_instance_id);
            println!("Operation: {}", view.report.operation_id);
            println!("Attempt: {}", view.report.attempt_id);
            display_report(view)?;
        }
        Observation::NotFound => unreachable!("Client 将 NotFound 返回为查询错误"),
    }
    Ok(())
}

fn display_report(observation: ReportObservation<String, String>) -> Result<()> {
    let report = observation.report;
    println!("Report: {}", report.report_id);
    match report.result {
        ExecutionResult::Succeeded { output } => {
            println!("状态: 成功; ImpactStatus: {:?}", report.impact);
            println!("输出: {output}");
        }
        ExecutionResult::Failed { error } => {
            bail!("执行失败; ImpactStatus: {:?}; 原因: {error}", report.impact)
        }
        ExecutionResult::Rejected { error } => bail!(
            "执行前拒绝; ImpactStatus: {:?}; 原因: {error}",
            report.impact
        ),
    }
    Ok(())
}

fn display_binding(binding: Binding) {
    println!("Operation: {}", binding.operation_id);
    println!("Attempt: {}", binding.attempt_id);
    println!(
        "状态: 已接纳; Authority={}; 不代表开始执行或完成",
        binding.authority_instance_id
    );
}
