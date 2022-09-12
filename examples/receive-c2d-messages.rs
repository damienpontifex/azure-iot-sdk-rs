use azure_iot_sdk::{DeviceKeyTokenSource, IoTHubClient, MessageType};
use log::info;

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

    let mut client = IoTHubClient::new(&hostname, device_id, token_source.into()).await?;

    info!("Initialized client");

    let mut recv = client.get_receiver().await;
    while let Some(msg) = recv.recv().await {
        if let MessageType::C2DMessage(msg) = msg {
            info!("Received message {:?}", msg)
        }
    }

    Ok(())
}
