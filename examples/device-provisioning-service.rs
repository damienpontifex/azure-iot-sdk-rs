#[macro_use]
extern crate log;

use azure_iot_sdk::{IoTHubClient, Message};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct DeviceConfig {
    scope_id: String,
    device_id: String,
    device_key: String,
}

impl DeviceConfig {
    fn from_env() -> Result<Self, config::ConfigError> {
        let mut cfg = config::Config::default();
        cfg.merge(config::File::with_name("examples/config"))?;
        cfg.try_into()
    }
}

#[tokio::main]
async fn main() -> azure_iot_sdk::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let config = DeviceConfig::from_env().unwrap();

    let mut client = IoTHubClient::from_provision_service(&config.scope_id, config.device_id.to_string(), &config.device_key, 5).await.unwrap();

    info!("Initialized client {:?}", client);

    for _ in 0..5 {
        let msg = Message::new(b"Hello, world!".to_vec());

        client.send_message(msg).await.expect("Failed to send message");
    }

    Ok(())
}
