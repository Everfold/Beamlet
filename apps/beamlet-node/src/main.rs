mod authority;
mod delivery;
mod ingress;
mod text_reverse;
mod worker;

use anyhow::{Context, Result, bail};
use std::env;

#[derive(Debug, PartialEq, Eq)]
enum Role {
    Authority,
    Worker,
}

/// 显式选择角色
fn role(args: &[String]) -> Result<Role> {
    match args {
        [flag, name] if flag == "--role" && name == "authority" => Ok(Role::Authority),
        [flag, name] if flag == "--role" && name == "worker" => Ok(Role::Worker),
        _ => bail!("用法: beamlet-node --role authority|worker"),
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let args: Vec<_> = env::args().skip(1).collect();
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
            let input = ingress::live::Inputs::connect(client.clone()).await?;
            let sender = delivery::nats::Delivery::new(client.clone(), delivery::REPORT_TIMEOUT);

            authority::run(input, sender, instance, tokio::signal::ctrl_c()).await?;
        }

        Role::Worker => {
            let inbox =
                delivery::nats::Inbox::subscribe(client.clone(), &[text_reverse::CAPABILITY])
                    .await?;

            worker::run(inbox, instance, tokio::signal::ctrl_c()).await?;
        }
    }

    client.flush().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
