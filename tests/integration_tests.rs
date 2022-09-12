use azure_iot_sdk::{DeviceKeyTokenSource, IoTHubClient};
use std::env;

fn test_config() -> (String, String, String) {
    (
        env::var("IOTHUB_HOSTNAME")
            .unwrap_or_else(|_| "azure-iot-sdk-rs.azure-devices.net".to_string()),
        env::var("IOT_DEVICE_ID").unwrap(),
        env::var("IOT_DEVICE_ACCESS_KEY").unwrap(),
    )
}

async fn client() -> azure_iot_sdk::Result<IoTHubClient> {
    let (hostname, device_id, access_key) = test_config();
    let token_source = DeviceKeyTokenSource::new(&hostname, &device_id, &access_key).unwrap();
    IoTHubClient::new(&hostname, device_id, token_source.into()).await
}

#[tokio::test]
async fn test_connect() {
    let client = client().await;
    assert!(client.is_ok());
}
