#[macro_use]
extern crate log;

use azure_iot_sdk::client::IoTHubClient;

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
async fn main() {
    env_logger::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let config = DeviceConfig::from_env().unwrap();

    let mut client = IoTHubClient::with_device_key(
        &config.hostname,
        &config.device_id,
        config.shared_access_key,
    )
    .await;

    info!("Initialized client");

    client
        .on_message(|msg| {
            println!(
                "Received message '{}'",
                std::str::from_utf8(&msg.body).unwrap()
            );
        })
        .await;

    std::thread::park();
}
