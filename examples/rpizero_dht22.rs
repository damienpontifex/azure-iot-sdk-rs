#[macro_use]
extern crate log;

use azure_iot_sdk::{client::IoTHubClient, message::Message};

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

const GPIO_DHT22: u8 = 4;

#[tokio::main]
async fn main() {
    let mut interval = time::interval(time::Duration::from_secs(5));

    loop {
        println!("{:?}", result);

        interval.tick().await;
    }

    env_logger::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let config = DeviceConfig::from_env().unwrap();

    let mut client =
        IoTHubClient::with_device_key(config.hostname, config.device_id, config.shared_access_key)
            .await;

    info!("Initialized client");

    let msg = Message::new(b"Hello, world!".to_vec());

    client.send_message(msg).await;
}
