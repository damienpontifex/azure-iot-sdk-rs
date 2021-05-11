use azure_iot_sdk::{IoTHubClient, Message};
use log::info;

#[tokio::main]
async fn main() -> azure_iot_sdk::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let scope_id = std::env::var("DPS_SCOPE_ID").expect(
        "Set the device provisioning service scope id in the DPS_SCOPE_ID environment variable",
    );
    let registration_id = std::env::var("DPS_REGISTRATION_ID").expect("Set the device provisioning service registration id in the DPS_REGISTRATION_ID environment variable");
    let device_key = std::env::var("DPS_DEVICE_KEY").expect(
        "Set the device provisioning service device key in the DPS_DEVICE_KEY environment variable",
    );

    let mut client =
        IoTHubClient::from_provision_service(&scope_id, registration_id, &device_key, 5)
            .await
            .unwrap();

    info!("Initialized client {:?}", client);

    for _ in 0..5 {
        let msg = Message::new(b"Hello, world!".to_vec());

        client
            .send_message(msg)
            .await
            .expect("Failed to send message");
    }

    Ok(())
}
