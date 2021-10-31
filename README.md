# Azure IoT SDK for Rust

Self developed library to interact with [Azure IoT Hub using MQTT protocol](https://docs.microsoft.com/en-us/azure/iot-hub/iot-hub-mqtt-support)

![CI](https://github.com/damienpontifex/azure-iot-sdk-rs/workflows/CI/badge.svg)
[![docs](https://docs.rs/azure_iot_sdk/badge.svg)](https://docs.rs/azure_iot_sdk)
[![Crate](https://img.shields.io/crates/v/azure_iot_sdk.svg)](https://crates.io/crates/azure_iot_sdk)
[![cratedown](https://img.shields.io/crates/d/azure_iot_sdk.svg)](https://crates.io/crates/azure_iot_sdk)
[![cratelastdown](https://img.shields.io/crates/dv/azure_iot_sdk.svg)](https://crates.io/crates/azure_iot_sdk)

## Usage

```rust
use azure_iot_sdk::{client::IoTHubClient, message::Message};
use log::info;

#[tokio::main]
async fn main() {
    env_logger::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let hostname = std::env::var("IOTHUB_HOSTNAME")
        .expect("Set IoT Hub hostname in the IOTHUB_HOSTNAME environment variable");
    let device_id = std::env::var("DEVICE_ID")
        .expect("Set the device id in the DEVICE_ID environment variable");
    let shared_access_key = std::env::var("SHARED_ACCESS_KEY")
        .expect("Set the device shared access key in the SHARED_ACCESS_KEY environment variable");

    let token_source = DeviceKeyTokenSource::new(
        &hostname,
        &device_id,
        &shared_access_key,
    )
    .unwrap();

    let mut client = IoTHubClient::new(&hostname, device_id, token_source.into()).await?;

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