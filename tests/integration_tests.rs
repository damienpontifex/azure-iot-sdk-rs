use azure_iot_sdk::{DeviceKeyTokenSource, IoTHubClient, MqttTransport};
use std::env;

fn test_config() -> (String, String, String) {
    (
        env::var("IOTHUB_HOSTNAME").unwrap_or("azure-iot-sdk-rs.azure-devices.net".to_string()),
        env::var("IOT_DEVICE_ID").unwrap(),
        env::var("IOT_DEVICE_ACCESS_KEY").unwrap(),
    )
}

async fn client() -> azure_iot_sdk::Result<IoTHubClient<MqttTransport>> {
    let (hostname, device_id, access_key) = test_config();
    let token_source = DeviceKeyTokenSource::new(&hostname, &device_id, &access_key).unwrap();
    IoTHubClient::<MqttTransport>::new(&hostname, device_id, token_source).await
}

#[tokio::test]
async fn test_connect() {
    let client = client().await;
    assert!(client.is_ok());
}
