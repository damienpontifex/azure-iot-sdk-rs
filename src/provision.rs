#[cfg(not(feature = "https-transport"))]
use std::collections::HashMap;

use chrono::{Duration, Utc};
#[cfg(feature = "https-transport")]
use hyper::{header, Body, Client, Method, Request, StatusCode};
#[cfg(feature = "https-transport")]
use hyper_tls::HttpsConnector;

#[cfg(not(feature = "https-transport"))]
use mqtt::{
    packet::*,
    Encodable, TopicName, {QualityOfService, TopicFilter},
};
#[cfg(not(feature = "https-transport"))]
use tokio::io::AsyncWriteExt;

#[cfg(not(feature = "https-transport"))]
use crate::mqtt_transport::mqtt_connect;

use crate::{
    client::IoTHubClient,
    token::{generate_token, DeviceKeyTokenSource},
};

#[cfg(not(feature = "https-transport"))]
use crate::Message;

const DPS_HOST: &str = "global.azure-devices-provisioning.net";
const DPS_API_VERSION: &str = "api-version=2019-03-31";

fn generate_registration_sas(
    scope: &str,
    device_id: &str,
    device_key: &str,
    expiry_timestamp: i64,
) -> String {
    let to_sign = format!(
        "{scope}%2fregistrations%2f{device_id}\n{expires}",
        scope = scope,
        device_id = device_id,
        expires = expiry_timestamp
    );

    let token = generate_token(device_key, &to_sign);

    let sas = format!(
        "SharedAccessSignature sr={scope}%2fregistrations%2f{device_id}&{token}&se={expires}",
        scope = scope,
        device_id = device_id,
        token = token,
        expires = expiry_timestamp
    );

    sas
}

#[derive(Debug)]
/// Errors that can be raised during provisioning or registration
pub enum ErrorKind {
    /// The Azure provisioning portal rejected your request for the device
    AzureProvisioningRejectedRequest,
    /// The provisioning service replied, but there was no operation Id
    ProvisionRequestReplyMissingOperationId,
    /// Timed out trying to get the IoT Hub for your device ID
    FailedToGetIotHub,
}

impl std::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("{:?}", self))
    }
}

impl std::error::Error for ErrorKind {}

#[derive(Debug)]
struct ProvisionedResponse {
    assigned_hub: String,
    device_id: String,
}

/// Use the device provision service to get our IoT Hubname
///
/// # Arguments
///
/// * `scope` - The scope ID to use for the registration call
/// * `registration_id` - The registered device to connect as
/// * `key` - The primary or secondary key for this device
/// * `max_retries` - The maximum number of retries at the provisioning service
///
/// Note that this uses the default Azure device provisioning
/// service, which may be blocked in some countries.
/// ```
#[cfg(all(feature = "with-provision", feature = "https-transport"))]
async fn get_iothub_from_provision_service(
    scope_id: &str,
    registration_id: &str,
    device_key: &str,
    max_retries: i32,
) -> Result<ProvisionedResponse, Box<dyn std::error::Error>> {
    let expiry = Utc::now() + Duration::days(1);
    let expiry = expiry.timestamp();
    let sas = generate_registration_sas(scope_id, registration_id, device_key, expiry);
    let url = format!(
        "https://{dps}/{scope_id}/registrations/{registration_id}/register?{api}",
        dps = DPS_HOST,
        scope_id = scope_id,
        registration_id = registration_id,
        api = DPS_API_VERSION
    );
    let body = serde_json::json!({
        "registrationId": registration_id,
    });
    let req = Request::builder()
        .method(Method::PUT)
        .uri(&url)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, sas.clone())
        .body(Body::from(body.to_string()))?;
    debug!("Request body {}", body);
    let https = HttpsConnector::new();
    let client = Client::builder().build::<_, hyper::Body>(https);
    let res = client.request(req).await?;
    if res.status() != StatusCode::ACCEPTED {
        let body = hyper::body::to_bytes(res).await.unwrap();
        error!("Rejection {:#?}", body);
        return Err(Box::new(ErrorKind::AzureProvisioningRejectedRequest));
    }

    // Extract retry-after response header for delay duration or default to 3s
    let retry_after = std::time::Duration::from_secs(
        res.headers()
            .get(header::RETRY_AFTER)
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.parse().ok())
            .unwrap_or(3),
    );

    let body = hyper::body::to_bytes(res).await.unwrap();
    let reply: serde_json::Map<String, serde_json::Value> = serde_json::from_slice(&body).unwrap();
    if !reply.contains_key("operationId") {
        return Err(Box::new(ErrorKind::ProvisionRequestReplyMissingOperationId));
    }
    let operation = reply.get("operationId").unwrap().as_str().unwrap();
    let url = format!(
        "{dps}/{scope_id}/registrations/{registration_id}/operations/{operation}?{api}",
        dps = DPS_HOST,
        scope_id = scope_id,
        registration_id = registration_id,
        operation = operation,
        api = DPS_API_VERSION
    );

    for _ in 0..max_retries {
        tokio::time::sleep(retry_after).await;

        let req = Request::builder()
            .method(Method::GET)
            .uri(&url)
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::AUTHORIZATION, sas.clone())
            .body(Body::empty())?;
        let res = client.request(req).await?;
        if res.status() == StatusCode::OK {
            let body = hyper::body::to_bytes(res).await.unwrap();
            let reply: serde_json::Map<String, serde_json::Value> =
                serde_json::from_slice(&body).unwrap();
            let registration_state = reply["registrationState"].as_object().unwrap();
            let hub = registration_state["assignedHub"].as_str().unwrap();
            return Ok(ProvisionedResponse {
                assigned_hub: hub.to_string(),
                device_id: registration_state["deviceId"].as_str().unwrap().to_string(),
            });
        }
    }

    Err(Box::new(ErrorKind::FailedToGetIotHub))
}

/// Use the device provision service to get our IoT Hubname
///
/// # Arguments
///
/// * `scope` - The scope ID to use for the registration call
/// * `registration_id` - The registered device to connect as
/// * `key` - The primary or secondary key for this device
/// * `max_retries` - The maximum number of retries at the provisioning service
///
/// Note that this uses the default Azure device provisioning
/// service, which may be blocked in some countries.
/// ```
#[cfg(all(feature = "with-provision", not(feature = "https-transport")))]
async fn get_iothub_from_provision_service(
    scope_id: &str,
    registration_id: &str,
    device_key: &str,
    max_retries: i32,
) -> Result<ProvisionedResponse, Box<dyn std::error::Error>> {
    let username = format!(
        "{}/registrations/{}/{}",
        scope_id, registration_id, DPS_API_VERSION
    );
    let expiry = Utc::now() + Duration::days(1);
    let expiry = expiry.timestamp();
    let sas = generate_registration_sas(scope_id, registration_id, device_key, expiry);
    let mut buf = Vec::new();
    let mut socket = mqtt_connect(&mut buf, DPS_HOST, registration_id, username, sas).await?;

    let topics = vec![(
        TopicFilter::new("$dps/registrations/res/#").unwrap(),
        QualityOfService::Level0,
    )];

    debug!("Subscribing to {:?}", topics);

    let subscribe_packet = SubscribePacket::new(10, topics);
    buf.clear();
    subscribe_packet.encode(&mut buf).unwrap();
    socket.write_all(&buf).await.unwrap();

    let register_topic = TopicName::new(format!(
        "$dps/registrations/PUT/iotdps-register/?$rid={request_id}",
        request_id = 1
    ))
    .unwrap();
    let device_registration = serde_json::json!({
        "registrationId": registration_id,
    });

    let publish_packet = PublishPacket::new(
        register_topic,
        QoSWithPacketIdentifier::Level0,
        device_registration.to_string(),
    );
    buf.clear();
    publish_packet.encode(&mut buf).unwrap();

    socket.write_all(&buf).await.unwrap();

    loop {
        let mut retries = 0;

        if let Ok(VariablePacket::PublishPacket(publ)) = VariablePacket::parse(&mut socket).await {
            debug!("PUBLISH {:?}", publ);
            let message = Message::new(publ.payload().to_vec());
            if publ.topic_name().starts_with("$dps/registrations/res/") {
                let body: serde_json::Value = serde_json::from_slice(&message.body).unwrap();

                // 200 Status response
                if Some(200)
                    == publ
                        .topic_name()
                        .split('/')
                        .nth_back(1)
                        .and_then(|s| s.parse::<u8>().ok())
                {
                    let registration_state = body["registrationState"].as_object().unwrap();
                    let hub = registration_state["assignedHub"].as_str().unwrap();
                    return Ok(ProvisionedResponse {
                        assigned_hub: hub.to_string(),
                        device_id: registration_state["deviceId"].as_str().unwrap().to_string(),
                    });
                }

                retries += 1;
                if retries > max_retries {
                    break;
                }

                let query_offset = publ.topic_name().find('?').unwrap();
                let property_tuples: HashMap<&str, &str> =
                    serde_urlencoded::from_str(&publ.topic_name()[query_offset..]).unwrap();

                let operation_id = body["operationId"].as_str().unwrap_or_default();

                if let Some(retry_after) = property_tuples
                    .get("retry-after")
                    .and_then(|r| r.parse::<u64>().ok())
                {
                    tokio::time::sleep(tokio::time::Duration::from_secs(retry_after)).await;
                }

                let topic_name = TopicName::new(format!("$dps/registrations/GET/iotdps-get-operationstatus/?$rid={request_id}&operationId={operation_id}", request_id = 1, operation_id = operation_id)).unwrap();
                let publish_packet =
                    PublishPacket::new(topic_name, QoSWithPacketIdentifier::Level0, "");
                buf.clear();
                publish_packet.encode(&mut buf).unwrap();

                socket.write_all(&buf).await.unwrap();
            }
        }
    }

    Err(Box::new(ErrorKind::FailedToGetIotHub))
}

impl IoTHubClient {
    /// Create a new IoT Hub device client using the device provisioning service
    ///
    /// # Arguments
    ///
    /// * `scope` - The scope ID to use for the registration call
    /// * `device_id` - The registered device to connect as
    /// * `key` - The primary or secondary key for this device
    /// * `max_retries` - The maximum number of retries at the provisioning service
    ///
    /// Note that this uses the default Azure device provisioning
    /// service, which may be blocked in some countries.
    ///
    /// # Example
    /// ```no_run
    /// use azure_iot_sdk::IoTHubClient;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let mut client = IoTHubClient::from_provision_service(
    ///           "ScopeID",
    ///           "DeviceID".into(),
    ///           "DeviceKey",
    ///           4).await;
    /// }
    /// ```
    #[cfg(feature = "with-provision")]
    pub async fn from_provision_service(
        scope_id: &str,
        device_id: String,
        device_key: &str,
        max_retries: i32,
    ) -> Result<IoTHubClient, Box<dyn std::error::Error>> {
        let response =
            get_iothub_from_provision_service(scope_id, &device_id, device_key, max_retries)
                .await?;

        debug!("Connecting to hub {:?} after provisioning", response);

        let token_source =
            DeviceKeyTokenSource::new(&response.assigned_hub, &response.device_id, device_key)
                .unwrap();
        let client = IoTHubClient::new(
            &response.assigned_hub,
            response.device_id,
            token_source.into(),
        )
        .await?;
        Ok(client)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provision_sas() {
        assert_eq!(generate_registration_sas("0ne000EEBBD", "FirstDevice", "O+H9VTcdJP0Tqkl7bh4nVG0OJNrAataMpuWB54D0VEc=", 1_591_921_306), "SharedAccessSignature sr=0ne000EEBBD%2fregistrations%2fFirstDevice&sig=hwgBlMB6G2Zg5ZcYtwmtLVKRbifiSCPfUMyscbVWa8o%3D&se=1591921306".to_string());
    }
}
