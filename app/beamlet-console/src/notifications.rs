//! Client 可选的只读生命周期通知订阅, 不发布 Client 生命周期消息

use anyhow::{Context, Result, ensure};
use beamlet::role_observation::Observations;
use beamlet_agent::testing::lifecycle::RoleNotice;
use beamlet_message_zenoh::Transport;

const MAX_INSTANCES: usize = 256;

pub(crate) async fn receive(transport: Transport) -> Result<()> {
    let mut retained = Observations::new(MAX_INSTANCES)?;
    loop {
        let message = match transport.recv_message().await {
            Ok(message) => message,
            Err(
                error @ (beamlet_message_zenoh::Error::UnexpectedDelete
                | beamlet_message_zenoh::Error::MessageTooLarge),
            ) => {
                eprintln!("已忽略角色生命周期通知: {error}");
                continue;
            }
            Err(error) => return Err(error).context("接收角色生命周期通知失败"),
        };
        let result = (|| {
            let notice = RoleNotice::from_json(&message.payload)?;
            ensure!(
                message.key_expr == notice.topic(),
                "角色生命周期通知路由与身份不符"
            );
            if !retained.record(notice.observation())? {
                return Ok(());
            }
            if let Some(outcome) = notice.exit_outcome() {
                println!(
                    "Client 收到 {} 退出通知: 逻辑身份={} 运行实例={} NodeId={} NodeInstanceId={}; 状态=已退出; 排空观察={outcome}; 不代表业务完成或外部效果停止",
                    notice.role,
                    notice.role_id,
                    notice.role_instance_id,
                    notice.node_id,
                    notice.node_instance_id
                );
            } else {
                println!(
                    "Client 收到 {} 启动公告: 逻辑身份={} 运行实例={} NodeId={} NodeInstanceId={}; 可为重播, 不代表当前在线",
                    notice.role,
                    notice.role_id,
                    notice.role_instance_id,
                    notice.node_id,
                    notice.node_instance_id
                );
            }
            Ok::<_, anyhow::Error>(())
        })();
        if let Err(error) = result {
            eprintln!("已忽略角色生命周期通知: {error}");
        }
    }
}
