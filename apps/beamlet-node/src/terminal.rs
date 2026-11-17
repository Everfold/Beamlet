//! Node 只读取一次 stdin, 将本地输入交给用户选定的 Authority.
use anyhow::{Context, Result};
use beamlet_api::RoleInstance;
use beamlet_host::Shutdown;
use std::io::BufRead;
use tokio::sync::mpsc;
use tracing::{info, warn};

/// 终端读取线程与路由循环之间的行缓冲, 独立于角色输入队列.
const INPUT_CAPACITY: usize = 16;

/// 终端行输入接收端; 每项保留文本读取结果, 发送端关闭表示 EOF.
pub(crate) type Input = mpsc::Receiver<std::io::Result<String>>;

/// 终端可选择的 Authority 目标及其有界文本输入发送端.
pub(crate) struct Target {
    /// 终端选择使用的完整 Authority 角色会话身份.
    pub instance: RoleInstance,
    /// 可选的人类可读标签, 不参与身份比较或消息路由.
    pub label: Option<String>,
    /// 只向该目标发送文本或 I/O 错误的有界输入端.
    pub send: mpsc::Sender<std::io::Result<String>>,
}

/// 为 Authority 终端目标启动统一 stdin 路由.
///
/// # 参数
///
/// - `targets`: 终端可选择的 Authority 目标列表及各自的输入发送端.
/// - `shutdown`: 停止通知或 future; 完成后停止新接收并按角色语义排空.
///
/// # 返回值
///
/// 无目标, EOF 或正常停止返回 `Ok(())`; 输入处理失败返回错误.
pub(crate) async fn run(targets: Vec<Target>, shutdown: Shutdown) -> Result<()> {
    if targets.is_empty() {
        return Ok(());
    }
    route(targets, input(), shutdown).await
}

/// 处理终端角色选择, 并把其他行发给唯一选中的 Authority.
///
/// # 参数
///
/// - `targets`: 终端可选择的 Authority 目标列表及各自的输入发送端.
/// - `input`: 统一 stdin 读取入口产生的文本或 I/O 错误队列.
/// - `shutdown`: 停止通知或 future; 完成后停止新接收并按角色语义排空.
///
/// # 返回值
///
/// EOF 或停止通知后返回 `Ok(())`; 读取失败返回错误; 队列满只记录拒绝并继续.
async fn route(targets: Vec<Target>, mut input: Input, shutdown: Shutdown) -> Result<()> {
    let mut selected = (targets.len() == 1).then_some(0);
    show(&targets);
    let shutdown = shutdown.wait();
    tokio::pin!(shutdown);
    loop {
        let line = tokio::select! {
            result = &mut shutdown => { result?; return Ok(()); }
            line = input.recv() => match line { Some(line) => line.context("读取本地输入失败")?, None => return Ok(()) },
        };
        if line == "/roles" {
            show(&targets);
            continue;
        }
        if let Some(value) = line.strip_prefix("/authority ") {
            selected = value
                .parse::<usize>()
                .ok()
                .and_then(|n| n.checked_sub(1))
                .filter(|&n| n < targets.len())
                .or_else(|| {
                    targets
                        .iter()
                        .position(|target| target.instance.role_id.to_string() == value)
                });
            if let Some(index) = selected {
                info!(role_id = %targets[index].instance.role_id, "已选择本地 Authority");
            } else {
                warn!("Authority 编号或角色 UUID 不存在");
            }
            continue;
        }
        let Some(index) = selected else {
            warn!("请先 /authority <编号或角色 UUID> 选择本地 Authority");
            continue;
        };
        // 满载只拒绝这次本地输入, 不阻塞切换角色或 Node 退出.
        if let Err(error) = targets[index].send.try_send(Ok(line)) {
            warn!(role_id = %targets[index].instance.role_id, %error, "本地 Authority 入口不可用");
        }
    }
}
/// 输出终端可选择的 Authority 角色列表.
///
/// # 参数
///
/// - `targets`: 终端可选择的 Authority 目标列表及各自的输入发送端.
///
/// # 返回值
///
/// `()`; 逐项输出编号, 角色和会话 UUID, 标签及选择提示.
fn show(targets: &[Target]) {
    for (index, target) in targets.iter().enumerate() {
        info!(index = index + 1, role_id = %target.instance.role_id, role_session_id = %target.instance.session_id,
            label = target.label.as_deref(), "本地 Authority; /authority <编号或角色 UUID> 选择, /roles 列出");
    }
}
/// 启动标准输入读取线程, 逐行转发读取结果.
///
/// # 返回值
///
/// 有界行输入接收端; 每项是文本或读取错误, EOF 后通道关闭.
fn input() -> Input {
    let (send, recv) = mpsc::channel(INPUT_CAPACITY);
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            if send.blocking_send(line).is_err() {
                break;
            }
        }
    });
    recv
}

/// 当前模块关键行为与边界条件的单元测试, 实现在 tests/unit 目录.
#[cfg(test)]
#[path = "../tests/unit/terminal.rs"]
mod tests;
