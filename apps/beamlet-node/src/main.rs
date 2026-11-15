//! 同一个程序按角色独立启动 Authority 或 Worker, 内部经 NATS 交付.
mod authority;
mod delivery;
mod ingress;
#[cfg(test)]
mod test_support;
mod worker;

use anyhow::{Context, Result, bail};
use std::env;

#[derive(Debug, PartialEq, Eq)]
enum Role {
    Authority,
    Worker,
}

/// 必须显式选择角色, 不隐式创建 Worker 或回退到本地执行.
fn role(args: &[String]) -> Result<Role> {
    match args {
        [flag, name] if flag == "--role" && name == "authority" => Ok(Role::Authority),
        [flag, name] if flag == "--role" && name == "worker" => Ok(Role::Worker),
        _ => bail!("用法: beamlet-node --role authority|worker"),
    }
}

/// 连接由程序入口组装, 实例身份由外层生成, 不绑定 kernel 的身份类型.
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args == ["--help"] {
        println!(
            "用法: beamlet-node --role authority|worker
NATS_URL 默认 nats://127.0.0.1:4222"
        );
        return Ok(());
    }
    let role = role(&args)?;
    let url = env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".into());
    let client = async_nats::ConnectOptions::new()
        .subscription_capacity(delivery::MAX_IN_FLIGHT)
        .connect(url)
        .await
        .context("连接 NATS 失败")?;
    let instance = uuid::Uuid::new_v4().to_string();
    match role {
        Role::Authority => {
            let sender = delivery::nats::Delivery::new(client.clone(), delivery::REPORT_TIMEOUT);
            let input = ingress::live::Inputs::connect(client.clone()).await?;
            authority::run(input, sender, instance, tokio::signal::ctrl_c()).await?;
        }
        Role::Worker => {
            let inbox =
                delivery::nats::Inbox::subscribe(client.clone(), &[delivery::REVERSE]).await?;
            worker::run(inbox, instance, tokio::signal::ctrl_c()).await?;
        }
    }
    client.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 无参数、旧入口或未知角色都不能悄悄启动另一种模式.
    #[test]
    fn requires_explicit_role() {
        assert_eq!(
            role(&["--role".into(), "authority".into()]).unwrap(),
            Role::Authority
        );
        assert_eq!(
            role(&["--role".into(), "worker".into()]).unwrap(),
            Role::Worker
        );
        for args in [
            vec![],
            vec!["--worker".into()],
            vec!["--demo".into()],
            vec!["--role".into(), "other".into()],
        ] {
            assert!(role(&args).is_err());
        }
    }
}
