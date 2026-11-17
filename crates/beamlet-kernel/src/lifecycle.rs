use state_machines::state_machine;

// 授权/报告循环：Ready → AwaitingReport → Ready；关闭：Ready → Closed。
state_machine! {
    name: Lifecycle,
    initial: Ready,
    states: [ Ready, AwaitingReport, Closed ],
    final_states: [ Closed ],
    events {
        create_attempt {
            transition: { from: Ready, to: AwaitingReport }
        }

        accept_report {
            transition: { from: AwaitingReport, to: Ready }
        }

        close {
            transition: { from: Ready, to: Closed }
        }
    }
}

/// 提取类型状态允许的无条件状态转换结果。
///
/// # 参数
///
/// - `result`：类型状态允许的无 guard/回调转换结果，按内部不变量应当成功。
///
/// # 返回值
///
/// 成功转换后的值；转换失败说明内部图不一致并触发 panic。
pub(crate) fn transition<T, E>(result: Result<T, E>) -> T {
    match result {
        Ok(machine) => machine,
        Err(_) => unreachable!("类型状态允许的无条件转换应成功"),
    }
}

/// 当前模块关键行为与边界条件的单元测试，实现在 tests/unit 目录。
#[cfg(test)]
#[path = "../tests/unit/lifecycle.rs"]
mod tests;
