use azure_iot_sdk::{DeviceKeyTokenSource, IoTHubClient, Message};
use log::info;
use serde::Deserialize;

#[tokio::main]
async fn main() -> azure_iot_sdk::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let hostname = std::env::var("IOTHUB_HOSTNAME")
        .expect("Set IoT Hub hostname in the IOTHUB_HOSTNAME environment variable");
    let device_id = std::env::var("DEVICE_ID")
        .expect("Set the device id in the DEVICE_ID environment variable");
    let shared_access_key = std::env::var("SHARED_ACCESS_KEY")
        .expect("Set the device shared access key in the SHARED_ACCESS_KEY environment variable");

    let token_source =
        DeviceKeyTokenSource::new(&hostname, &device_id, &shared_access_key).unwrap();

    let mut client = IoTHubClient::new(&hostname, device_id, token_source).await?;

    info!("Initialized client");

    for _ in 0..5 {
        let msg = Message::new(b"Hello, world!".to_vec());

        client
            .send_message(msg)
            .await
            .expect("Failed to send message");
    }

    Ok(())
}
