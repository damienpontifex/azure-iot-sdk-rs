use chrono::{Duration, Utc};
use hyper::{header, Body, Client, Method, Request, StatusCode};
use hyper_tls::HttpsConnector;

use crate::{
    client::IoTHubClient,
    token::{generate_token, DeviceKeyTokenSource},
};

const DPS_HOST: &str = "https://global.azure-devices-provisioning.net";
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

/// Use the device provision service to get our IoT Hubname
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
/// use azure_iot_sdk::provision::get_iothub_from_provision_service;
///
/// #[tokio::main]
/// async fn main() {
///
/// let mut client = get_iothub_from_provision_service(
///           "ScopeID",
///           "DeviceID",
///           "DeviceKey",
///           4).await;
/// }
/// ```
#[cfg(feature = "with-provision")]
pub async fn get_iothub_from_provision_service(
    scope_id: &str,
    device_id: &str,
    device_key: &str,
    max_retries: i32,
) -> Result<String, Box<dyn std::error::Error>> {
    let expiry = Utc::now() + Duration::days(1);
    let expiry = expiry.timestamp();
    let sas = generate_registration_sas(scope_id, device_id, device_key, expiry);
    let url = format!(
        "{dps}/{scope_id}/registrations/{device_id}/register?{api}",
        dps = DPS_HOST,
        scope_id = scope_id,
        device_id = device_id,
        api = DPS_API_VERSION
    );
    let mut map = serde_json::Map::new();
    map.insert(
        "registrationId".to_string(),
        serde_json::Value::String(device_id.to_string()),
    );
    let req = Request::builder()
        .method(Method::PUT)
        .uri(&url)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, sas.clone())
        .body(Body::from(serde_json::to_string(&map).unwrap()))?;
    trace!(
        "Request body {}",
        serde_json::to_string_pretty(&map).unwrap()
    );
    let https = HttpsConnector::new();
    let client = Client::builder().build::<_, hyper::Body>(https);
    let res = client.request(req).await?;
    if res.status() != StatusCode::ACCEPTED {
        let body = hyper::body::to_bytes(res).await.unwrap();
        error!("Rejection {:#?}", body);
        return Err(Box::new(ErrorKind::AzureProvisioningRejectedRequest));
    }

    // Extract retry-after response header for delay duration or default to 3s
    let retry_after = std::time::Duration::from_secs(res
        .headers()
        .get(header::RETRY_AFTER)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.parse().ok())
        .unwrap_or(3));

    let body = hyper::body::to_bytes(res).await.unwrap();
    let reply: serde_json::Map<String, serde_json::Value> = serde_json::from_slice(&body).unwrap();
    if !reply.contains_key("operationId") {
        return Err(Box::new(ErrorKind::ProvisionRequestReplyMissingOperationId));
    }
    let operation = reply.get("operationId").unwrap().as_str().unwrap();
    let url = format!(
        "{dps}/{scope_id}/registrations/{device_id}/operations/{operation}?{api}",
        dps = DPS_HOST,
        scope_id = scope_id,
        device_id = device_id,
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
            return Ok(hub.to_string());
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
        let hubname =
            get_iothub_from_provision_service(scope_id, &device_id, device_key, max_retries)
                .await?;

        let token_source = DeviceKeyTokenSource::new(&hubname, &device_id, device_key).unwrap();
        let client = IoTHubClient::new(&hubname, device_id, token_source).await?;
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
