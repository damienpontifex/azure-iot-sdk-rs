use azure_iot_sdk::{DeviceKeyTokenSource, DirectMethodResponse, IoTHubClient, MessageType};
use iothub::service::ServiceClient;
use serde_json::json;
use std::env;

fn test_config() -> (String, String, String) {
    (
        env::var("IOTHUB_HOSTNAME").unwrap_or("azure-iot-sdk-rs.azure-devices.net".to_string()),
        env::var("IOT_DEVICE_ID").unwrap(),
        env::var("IOT_DEVICE_ACCESS_KEY").unwrap(),
    )
}

async fn client() -> azure_iot_sdk::Result<IoTHubClient> {
    let (hostname, device_id, access_key) = test_config();
    let token_source = DeviceKeyTokenSource::new(&hostname, &device_id, &access_key).unwrap();
    IoTHubClient::new(&hostname, device_id, token_source).await
}

#[tokio::test]
async fn test_connect() {
    let client = client().await;
    assert!(client.is_ok());
}

#[tokio::test]
async fn test_direct_method() {
    let service_client = ServiceClient::from_connection_string("", 3600).unwrap();

    let direct_method = service_client.create_device_method("device_id", "method_name", 30, 30);

    let mut client = client().await.unwrap();
    let mut receiver = client.get_receiver().await;
    tokio::task::spawn(async move {
        loop {
            while let Some(msg) = receiver.recv().await {
                match msg {
                    MessageType::DirectMethod(msg) => {
                        client
                            .respond_to_direct_method(DirectMethodResponse::new(
                                msg.request_id,
                                0,
                                Some("test_direct_method response".to_string()),
                            ))
                            .await
                            .unwrap();
                    }
                    _ => println!("Received other event"),
                }
            }
        }
    });

    let response = direct_method.execute(json!({ "key": "value" })).await;
    assert!(response.is_ok());
    let response = response.unwrap();
    assert_eq!(response.status, 0);
    assert_eq!(
        response.payload,
        Some("test_direct_method response".to_string())
    );
}
