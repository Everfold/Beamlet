use state_machines::state_machine;

state_machine! {
    name: Lifecycle,
    initial: Ready,
    states: [ Ready, AwaitingReport, Closed ],
    events {
        create_attempt {
            transition: { from: Ready, to: AwaitingReport }
        }
        accept_report {
            transition: { from: AwaitingReport, to: Ready }
        }
        close {
            transition: { from: [ Ready, Closed ], to: Closed }
        }
    }
}

/// 提取无 guard 和回调的合法转换, 失败表示内部状态图不一致.
pub(crate) fn transition<T, E>(result: Result<T, E>) -> T {
    match result {
        Ok(machine) => machine,
        Err(_) => unreachable!("类型状态允许的无条件转换应成功"),
    }
}
