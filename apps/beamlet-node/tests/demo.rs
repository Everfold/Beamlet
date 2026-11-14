use std::{process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
    time::timeout,
};

/// 从演示输出读取 PID, 缺失字段表示进程流程没有完成.
fn pid(output: &str, field: &str) -> u32 {
    output
        .lines()
        .find_map(|line| line.strip_prefix(field))
        .expect("演示应输出进程 PID")
        .parse()
        .expect("PID 应为整数")
}

/// 不同函数乱序返回时, Authority 仍关闭正确的 Operation 且结果不串线.
#[tokio::test]
async fn demo_reuses_worker_and_closes_both_operations() {
    let mut command = Command::new(env!("CARGO_BIN_EXE_beamlet-node"));
    command.kill_on_drop(true);
    // 测试期限稍长于程序自身的十秒期限, 便于先收到程序的超时错误.
    let output = timeout(Duration::from_secs(15), command.output())
        .await
        .expect("演示不应挂起")
        .expect("启动演示");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let authority = pid(&stdout, "[管理进程] 进程号=");
    let worker = pid(&stdout, "[管理进程] 工作进程号=");
    // 分别检查角色之间及角色与测试进程的 PID, 避免把线程模拟误当成进程隔离.
    assert_ne!(authority, worker);
    assert_ne!(authority, std::process::id());
    assert_ne!(worker, std::process::id());
    let reports: Vec<_> = stdout
        .lines()
        .filter(|line| line.starts_with("[管理进程] 调用完成:"))
        .collect();
    assert_eq!(
        reports,
        [
            format!(
                "[管理进程] 调用完成: 操作=2, 授权=102, 报告=2, 工作进程号={worker}, 返回值=HELLO BEAMLET, 状态=已关闭."
            ),
            format!(
                "[管理进程] 调用完成: 操作=1, 授权=101, 报告=1, 工作进程号={worker}, 返回值=Hello World, 状态=已关闭."
            ),
        ]
    );
}

/// 管道保持打开时逐次得到报告, EOF 后 Worker 正常退出且没有额外消息.
#[tokio::test]
async fn worker_replies_before_eof_and_preserves_each_identity() {
    timeout(Duration::from_secs(15), async {
        let mut child = Command::new(env!("CARGO_BIN_EXE_beamlet-node"))
            .arg("--worker").stdin(Stdio::piped()).stdout(Stdio::piped())
            .kill_on_drop(true).spawn().unwrap();
        let worker_pid = child.id().unwrap();
        let mut input = child.stdin.take().unwrap();
        let mut replies = BufReader::new(child.stdout.take().unwrap()).lines();
        for (operation_id, attempt_id, report_id) in [(7, 41, 1), (9, 53, 2)] {
            let call = format!(
                "{{\"operation_id\":{operation_id},\"attempt_id\":{attempt_id},\"request\":{{\"function\":\"hello_world\"}}}}\n"
            );
            // 拆成两次写入, 验证消息边界由换行决定而不是单次 write.
            let split = call.len() / 2;
            input.write_all(&call.as_bytes()[..split]).await.unwrap();
            input.write_all(&call.as_bytes()[split..]).await.unwrap();
            let line = replies.next_line().await.unwrap().expect("管道关闭前应收到报告");
            let reply: serde_json::Value = serde_json::from_str(&line).unwrap();
            assert_eq!(reply, serde_json::json!({
                "operation_id": operation_id,
                "attempt_id": attempt_id,
                "report_id": report_id,
                "worker_pid": worker_pid,
                "result": "Hello World",
            }));
            assert!(child.try_wait().unwrap().is_none(), "Worker 应等待下一次调用");
        }
        drop(input);
        assert!(replies.next_line().await.unwrap().is_none());
        assert!(child.wait().await.unwrap().success());
    }).await.expect("Worker 应逐次回复并在 EOF 后退出");
}

/// 连续发送不同参数后立即关闭输入, Worker 仍完整返回乱序报告.
#[tokio::test]
async fn worker_drains_concurrent_calls_with_distinct_payloads() {
    timeout(Duration::from_secs(15), async {
        let mut child = Command::new(env!("CARGO_BIN_EXE_beamlet-node"))
            .arg("--worker")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let worker_pid = child.id().unwrap();
        let mut input = child.stdin.take().unwrap();
        let calls = [
            serde_json::json!({
                "operation_id": 7, "attempt_id": 41, "delay_ms": 300,
                "request": {"function": "uppercase", "arguments": {"text": "first"}}
            }),
            serde_json::json!({
                "operation_id": 9, "attempt_id": 53,
                "request": {"function": "uppercase", "arguments": {"text": "你好\nbeamlet"}}
            }),
        ];
        for call in calls {
            let mut bytes = serde_json::to_vec(&call).unwrap();
            bytes.push(b'\n');
            input.write_all(&bytes).await.unwrap();
        }
        // EOF 发生在任务完成前, 验证它不会取消在途调用.
        drop(input);
        let output = child.wait_with_output().await.unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        let reports: Vec<serde_json::Value> = stdout
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(
            reports,
            [
                serde_json::json!({
                    "operation_id": 9, "attempt_id": 53, "report_id": 2,
                    "worker_pid": worker_pid, "result": "你好\nBEAMLET"
                }),
                serde_json::json!({
                    "operation_id": 7, "attempt_id": 41, "report_id": 1,
                    "worker_pid": worker_pid, "result": "FIRST"
                }),
            ]
        );
    })
    .await
    .expect("输入关闭后应收齐在途报告");
}

/// 缺少函数要求的参数时拒绝调用, 不生成成功报告.
#[tokio::test]
async fn worker_rejects_missing_function_arguments() {
    timeout(Duration::from_secs(15), async {
        let mut child = Command::new(env!("CARGO_BIN_EXE_beamlet-node"))
            .arg("--worker")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let mut input = child.stdin.take().unwrap();
        let call = serde_json::json!({
            "operation_id": 1, "attempt_id": 1,
            "request": {"function": "uppercase", "arguments": {}}
        });
        input
            .write_all(format!("{call}\n").as_bytes())
            .await
            .unwrap();
        drop(input);
        let output = child.wait_with_output().await.unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("解析调用失败"));
    })
    .await
    .expect("非法请求应及时拒绝");
}

/// Worker 接口拒绝未知函数, 不输出看似成功的报告.
#[tokio::test]
async fn worker_rejects_unknown_function() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_beamlet-node"))
        .arg("--worker")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    input
        .write_all(
            b"{\"operation_id\":1,\"attempt_id\":1,\"request\":{\"function\":\"unknown\"}}\n",
        )
        .await
        .unwrap();
    // 发送后关闭管道, 即使解码失败也不会因等待后续输入而阻塞测试.
    drop(input);
    let output = timeout(Duration::from_secs(15), child.wait_with_output())
        .await
        .unwrap()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("解析调用失败"));
}
