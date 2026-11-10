//! Console 提交固定 Hello 请求, 按 Operation 身份显示已收录的执行证据

use std::{env, time::Duration};

use anyhow::{Context, Result, bail, ensure};
use beamlet::{
    id::SubmissionKey,
    model::ExecutionResult,
    remote::{Binding, Client, ClientError, Outcome},
};
use beamlet_agent::testing::{hello::Task, submission::HelloSubmissionCodec};
use beamlet_message_zenoh::{Limits, SubmissionTransport, Transport};
use tokio::{
    io::{self, AsyncBufReadExt, AsyncReadExt, BufReader},
    time::timeout,
};
use uuid::Uuid;

mod notifications;
const NODE_INBOX: &str = "beamlet/v0/node/inbox";
const ROUND_TRIP_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_LINE_BYTES: usize = 64 * 1024;
type HelloClient<'a> = Client<SubmissionTransport<'a>, HelloSubmissionCodec>;

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
            "--demo" => return Ok(beamlet_agent::testing::demo::print().await?),
            "--help" | "-h" => {
                println!(
                    "用法: beamlet-console [--connect tcp/127.0.0.1:7447] [--authority UUID] [--subscribe] [--message hello [--submission-key UUID]] [--demo]\n--subscribe 启用只读角色启动/退出通知; 输入结束后继续订阅, Ctrl-C 退出"
                );
                return Ok(());
            }
            _ => bail!("未知参数: {arg}"),
        }
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
    let destination = match authority {
        Some(authority) => format!(
            "beamlet/v0/authority/{}/submit/{console_id}",
            authority.simple()
        ),
        None => format!("{NODE_INBOX}/{console_id}"),
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
                .subscribe(beamlet_agent::testing::lifecycle::SUBSCRIPTION)
                .await?,
        )
    } else {
        None
    };
    let channel = SubmissionTransport::bind(&mut transport, &destination).await?;
    if !subscribe {
        timeout(ROUND_TRIP_TIMEOUT, channel.wait_for_receiver())
            .await
            .context("Node 订阅尚未就绪")??;
    }
    let mut client = Client::new(
        channel,
        HelloSubmissionCodec::default(),
        authority.map(|id| id.as_u128().into()),
    );
    println!("Console 已连接: {endpoint} (会话 {console_id})");
    let business = async {
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
        Outcome::Report(observation) => {
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
        }
        Outcome::Pending => println!("状态: 待定; 不创建新授权"),
        Outcome::SubmissionError(error) => bail!("提交返回错误, 不是 Report: {error}"),
        _ => bail!("不支持的结果观察"),
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
