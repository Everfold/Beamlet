mod authority;
mod delivery;
mod ingress;
mod settings;
mod text_reverse;
mod worker;

use anyhow::{Context, Result};
use settings::{Role, Settings};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let settings = Settings::load()?;

    let client = async_nats::ConnectOptions::new()
        .subscription_capacity(delivery::MAX_IN_FLIGHT)
        .connect(settings.nats_url)
        .await
        .context("连接 NATS 失败")?;

    let instance = uuid::Uuid::new_v4().to_string();
    match settings.role {
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
