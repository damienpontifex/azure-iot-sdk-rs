# Azure IoT SDK for Rust

Self developed library to interact with [Azure IoT Hub using MQTT protocol](https://docs.microsoft.com/en-us/azure/iot-hub/iot-hub-mqtt-support)

![CI](https://github.com/damienpontifex/azure-iot-sdk-rs/workflows/CI/badge.svg)
[![docs](https://docs.rs/azure_iot_sdk/badge.svg)](https://docs.rs/azure_iot_sdk)
[![Crate](https://img.shields.io/crates/v/azure_iot_sdk.svg)](https://crates.io/crates/azure_iot_sdk)
[![cratedown](https://img.shields.io/crates/d/azure_iot_sdk.svg)](https://crates.io/crates/azure_iot_sdk)
[![cratelastdown](https://img.shields.io/crates/dv/azure_iot_sdk.svg)](https://crates.io/crates/azure_iot_sdk)

## Running examples
Copy the sample config file
```bash
cp examples/config.sample.toml examples/config.toml
```

Edit values in examples/config.toml with your iot hub host, device and primary key

## Usage

```rust
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

#[tokio::main]
async fn main() {
    env_logger::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let config = DeviceConfig::from_env().unwrap();

    let token_source = DeviceKeyTokenSource::new(
        &config.hostname,
        &config.device_id,
        &config.shared_access_key,
    )
    .unwrap();

    let mut client =
        IoTHubClient::new(&config.hostname, config.device_id.clone(), token_source).await?;

    info!("Initialized client");

    let mut recv = client.get_receiver().await;
    let receive_loop = async {
        while let Some(msg) = recv.recv().await {
            match msg {
                MessageType::C2DMessage(msg) => info!("Received message {:?}", msg),
                _ => {}
            }
        }
    };

    let msg = Message::new(b"Hello, world!".to_vec());
    let sender = client.send_message(msg);

    tokio::join!(receive_loop, sender);
}
```