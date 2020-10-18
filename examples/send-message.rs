#[macro_use]
extern crate log;

use azure_iot_sdk::{DeviceKeyTokenSource, IoTHubClient, Message, MqttTransport};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct DeviceConfig {
    hostname: String,
    device_id: String,
    shared_access_key: String,
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

    let token_source = DeviceKeyTokenSource::new(
        &config.hostname,
        &config.device_id,
        &config.shared_access_key,
    )
    .unwrap();

    let mut client =
        IoTHubClient::<MqttTransport>::new(&config.hostname, &config.device_id, token_source)
            .await?;

    info!("Initialized client");

    let msg = Message::new(b"Hello, world!".to_vec());

    client.send_message(msg).await
}
