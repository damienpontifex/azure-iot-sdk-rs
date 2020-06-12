use chrono::{Duration, Utc};
use hmac::{Hmac, Mac};
#[cfg(feature = "with-provision")]
use hyper::{Body, Client, header, Method, Request};
use hyper::header::AUTHORIZATION;
use hyper::StatusCode;
#[cfg(feature = "with-provision")]
use hyper_tls::HttpsConnector;
use sha2::Sha256;

use crate::errors::ErrorKind;
#[cfg(feature = "http-transport")]
use crate::http_transport::HttpTransport;
use crate::message::Message;
#[cfg(not(any(feature = "http-transport", feature = "amqp-transport")))]
use crate::mqtt_transport::MqttTransport;
use crate::transport::{MessageHandler, Transport};

const DEVICEID_KEY: &str = "DeviceId";
const HOSTNAME_KEY: &str = "HostName";
const SHAREDACCESSKEY_KEY: &str = "SharedAccessKey";

/// Client for communicating with IoT hub
#[derive(Debug, Clone)]
pub struct IoTHubClient {
    device_id: String,
    #[cfg(not(any(feature = "http-transport", feature = "amqp-transport")))]
    transport: MqttTransport,
    #[cfg(feature = "http-transport")]
    transport: HttpTransport,
}

fn generate_token(key: &str, message: &str) -> String {
    let key = base64::decode(&key).unwrap();
    let mut mac = Hmac::<Sha256>::new_varkey(&key).unwrap();
    mac.input(message.as_bytes());
    let mac_result = mac.result().code();
    let signature = base64::encode(mac_result.as_ref());

    let pairs = &vec![("sig", signature)];
    serde_urlencoded::to_string(pairs).unwrap()
}

fn generate_sas(hub: &str, device_id: &str, key: &str, expiry_timestamp: i64) -> String {
    let resource_uri = format!("{}/devices/{}", hub, device_id);

    const FRAGMENT: &percent_encoding::AsciiSet = &percent_encoding::CONTROLS.add(b'/');

    let resource_uri = percent_encoding::utf8_percent_encode(&resource_uri, FRAGMENT);
    let to_sign = format!("{}\n{}", &resource_uri, expiry_timestamp);

    let token = generate_token(key, &to_sign);

    let sas = format!(
        "SharedAccessSignature sr={}&{}&se={}",
        resource_uri, token, expiry_timestamp
    );

    sas
}

fn generate_registration_sas(scope: &str, device_id: &str, device_key: &str, expiry_timestamp: i64) -> String {
    let to_sign = format!("{scope}%2fregistrations%2f{device_id}\n{expires}", scope = scope,
                          device_id = device_id, expires = expiry_timestamp);

    let token = generate_token(device_key, &to_sign);

    let sas = format!("SharedAccessSignature sr={scope}%2fregistrations%2f{device_id}&{token}&se={expires}",
                      scope = scope, device_id = device_id, token = token,
                      expires = expiry_timestamp);

    sas
}

impl IoTHubClient {
    /// Create a new IoT Hub device client using the device's primary key
    ///
    /// # Arguments
    ///
    /// * `hub` - The IoT hub resource name
    /// * `device_id` - The registered device to connect as
    /// * `key` - The primary or secondary key for this device
    ///
    /// # Example
    /// ```no_run
    /// use azure_iot_sdk::client::IoTHubClient;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let mut client = IoTHubClient::with_device_key(
    ///         "iothubname.azure-devices.net".into(),
    ///         "MyDeviceId".into(),
    ///         "TheAccessKey".into()).await;
    /// }
    /// ```
    pub async fn with_device_key(hub: String, device_id: String, key: String) -> Self {
        let expiry = Utc::now() + Duration::days(1);
        let expiry = expiry.timestamp();

        let sas = generate_sas(&hub, &device_id, &key, expiry);

        Self::new(hub, device_id, sas).await
    }

    /// Create a new IoT Hub device client using the device's connection string
    ///
    /// # Arguments
    ///
    /// * `connection_string` - The connection string for this device and iot hub
    ///
    /// # Example
    /// ```no_run
    /// use azure_iot_sdk::client::IoTHubClient;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let mut client = IoTHubClient::from_connection_string(
    ///         "HostName=iothubname.azure-devices.net;DeviceId=MyDeviceId;SharedAccessKey=TheAccessKey").await;
    /// }
    /// ```
    pub async fn from_connection_string(connection_string: &str) -> Self {
        let mut key = None;
        let mut device_id = None;
        let mut hub = None;

        let parts: Vec<&str> = connection_string.split(';').collect();
        for p in parts {
            let s: Vec<&str> = p.split('=').collect();
            match s[0] {
                SHAREDACCESSKEY_KEY => key = Some(s[1].to_string()),
                DEVICEID_KEY => device_id = Some(s[1].to_string()),
                HOSTNAME_KEY => hub = Some(s[1].to_string()),
                _ => (), // Ignore extraneous component in the connection string
            }
        }

        // let key = key.ok_or(ErrorKind::ConnectionStringMissingRequiredParameter(
        //     SHAREDACCESSKEY_KEY,
        // ))?;

        Self::with_device_key(hub.unwrap(), device_id.unwrap(), key.unwrap()).await
    }

    /// Create a new IoT Hub device client using a shared access signature
    ///
    /// # Arguments
    ///
    /// * `hub_name` - The IoT hub resource name
    /// * `device_id` - The registered device to connect as
    /// * `sas` - The shared access signature for this device to connect with
    ///
    /// # Example
    /// ```no_run
    /// use azure_iot_sdk::client::IoTHubClient;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let mut client = IoTHubClient::new(
    ///         "iothubname.azure-devices.net".into(),
    ///         "MyDeviceId".into(),
    ///         "SharedAccessSignature sr=iothubname.azure-devices.net%2Fdevices%2MyDeviceId&sig=vn0%2BgyIUKgaBhEU0ypyOhJ0gPK5fSY1TKdvcJ1HxhnQ%3D&se=1587123309".into()).await;
    /// }
    /// ```
    pub async fn new(hub_name: String, device_id: String, sas: String) -> Self {
        #[cfg(not(any(feature = "http-transport", feature = "amqp-transport")))]
        let transport = MqttTransport::new(hub_name, device_id.clone(), sas).await;

        #[cfg(feature = "http-transport")]
            let transport = HttpTransport::new(hub_name, device_id.clone(), sas).await;

        Self {
            device_id,
            transport,
        }
    }

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
    /// use azure_iot_sdk::client::IoTHubClient;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let mut client = IoTHubClient::from_provision_service(
    ///           "ScopeID".into(),
    ///           "DeviceID".into(),
    ///           "DeviceKey".into(),
    ///           4).await;
    /// }
    /// ```
    #[cfg(feature = "with-provision")]
    pub async fn from_provision_service(scope_id: &str, device_id: &str, device_key: &str, max_retries: i32) -> Result<IoTHubClient, Box<dyn std::error::Error>> {
        let expiry = Utc::now() + Duration::days(1);
        let expiry = expiry.timestamp();
        let dps = "https://global.azure-devices-provisioning.net";
        let api = "api-version=2018-11-01";
        let sas = generate_registration_sas(scope_id, device_id, device_key, expiry);
        let url = format!("{dps}/{scope_id}/registrations/{device_id}/register?{api}",
                          dps = dps, scope_id = scope_id, device_id = device_id, api = api);
        let mut map = serde_json::Map::new();
        map.insert("registrationId".to_string(), serde_json::Value::String(device_id.to_string()));
        let req = Request::builder()
            .method(Method::PUT)
            .uri(&url)
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::AUTHORIZATION, sas.clone())
            .body(Body::from(serde_json::to_string(&map).unwrap()))?;
        let https = HttpsConnector::new();
        let client = Client::builder().build::<_, hyper::Body>(https);
        let res = client.request(req).await?;
        if res.status() != StatusCode::ACCEPTED {
            return Err(Box::new(ErrorKind::AzureProvisioningRejectedRequest))
        }
        let body = hyper::body::to_bytes(res).await.unwrap();
        let reply: serde_json::Map<String, serde_json::Value> = serde_json::from_slice(&body).unwrap();
        if !reply.contains_key("operationId") {
            return Err(Box::new(ErrorKind::ProvisionRequestReplyMissingOperationId))
        }
        let operation = reply.get("operationId").unwrap().as_str().unwrap();
        let url = format!("{dps}/{scope_id}/registrations/{device_id}/operations/{operation}?{api}",
                          dps = dps, scope_id = scope_id, device_id = device_id,
                          operation = operation, api = api);
        let mut retries: i32 = 0;
        let mut hubname = String::new();
        while retries < max_retries {
            tokio::time::delay_for(std::time::Duration::from_secs(3)).await;
            let req = Request::builder()
                .method(Method::GET)
                .uri(&url)
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, sas.clone())
                .body(Body::empty())?;
            let res = client.request(req).await?;
            if res.status() == StatusCode::OK {
                let body = hyper::body::to_bytes(res).await.unwrap();
                let reply: serde_json::Map<String, serde_json::Value> = serde_json::from_slice(&body).unwrap();
                let registration_state = reply["registrationState"].as_object().unwrap();
                let hub = registration_state["assignedHub"].as_str().unwrap();
                hubname = hub.to_string();
                break;
            }
            retries += 1;
        }
        if hubname.is_empty() {
            return Err(Box::new(ErrorKind::FailedToGetIotHub));
        }
        Ok(IoTHubClient::with_device_key(hubname, device_id.to_string(), device_key.to_string()).await)
    }

    /// Send a device to cloud message for this device to the IoT Hub
    ///
    /// # Example
    /// ```no_run
    /// use azure_iot_sdk::client::IoTHubClient;
    /// use azure_iot_sdk::message::Message;
    /// use tokio::time;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let mut client = IoTHubClient::with_device_key(
    ///         "iothubname.azure-devices.net".into(),
    ///         "MyDeviceId".into(),
    ///         "TheAccessKey".into()).await;
    ///
    ///     let mut interval = time::interval(time::Duration::from_secs(1));
    ///     let mut count: u32 = 0;
    ///
    ///     loop {
    ///         interval.tick().await;
    ///
    ///         let msg = Message::builder()
    ///             .set_body(format!("Message #{}", count).as_bytes().to_vec())
    ///             .set_message_id(format!("{}-t", count))
    ///             .build();
    ///
    ///         client.send_message(msg).await;
    ///
    ///         count += 1;
    ///     }
    /// }
    /// ```
    pub async fn send_message(&mut self, message: Message) {
        self.transport.send_message(message).await;
    }

    /// Send a property update from the device to the cloud
    ///
    /// Property updates sent from the device are used to publish the
    /// device's current values for "properties" in IoTCentral terminology
    /// or Device Twin Attributes in IoTHub terminology.  The body of the
    /// message should be JSON encoded with a map of names to values.  The
    /// request ID should be a unique ID that will match the response sent
    /// from the server via the property channel.
    ///
    /// # Example
    ///
    /// Suppose we have two properties `property_1` and `property_2` defined on our Device Twin
    /// (or defined as properties in our IoTCentral device capability model).  For convenience
    /// we define a struct so we can use `serde` to convert them to JSON.
    ///
    /// ```ignore
    /// #[derive(Serialize)]
    /// struct MyProperties {
    ///    property_1: f64,
    ///    property_2: f64,
    /// }
    /// ```
    ///
    /// Then to send the current value of the properties to the cloud, we would use something like
    ///
    /// ```ignore
    ///    let my_struct = MyProperties {property_1 : 31.0, property_2: 42.0};
    ///    let body = serde_json::to_string(&my_struct).unwrap();
    ///    client.send_property_update(&format!("{}", update_counter), &body).await;
    ///    update_counter += 1;
    /// ```
    pub async fn send_property_update(&mut self, request_id: &str, body: &str) {
        self.transport.send_property_update(request_id, body).await;
    }

    /// Define the cloud to device message handler
    ///
    /// # Example
    /// ```no_run
    /// use azure_iot_sdk::client::IoTHubClient;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let mut client = IoTHubClient::with_device_key(
    ///         "iothubname.azure-devices.net".into(),
    ///         "MyDeviceId".into(),
    ///         "TheAccessKey".into()).await;
    ///
    ///     client
    ///        .on_message(|msg| {
    ///            println!("Received message {:?}", msg);
    ///        })
    ///        .await;
    /// }
    /// ```
    #[cfg(feature = "c2d-messages")]
    pub async fn on_message<T>(&mut self, handler: T)
    where
        T: Fn(Message) + Send + 'static,
    {
        self.transport
            .set_message_handler(&self.device_id, MessageHandler::Message(Box::new(handler)))
            .await;
    }

    /// Define the message handler for direct method invocation
    ///
    /// # Example
    /// ```no_run
    /// use azure_iot_sdk::client::IoTHubClient;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let mut client = IoTHubClient::with_device_key(
    ///         "iothubname.azure-devices.net".into(),
    ///         "MyDeviceId".into(),
    ///         "TheAccessKey".into()).await;
    ///
    ///     client
    ///        .on_direct_method(|method_name, msg| {
    ///             println!("Received direct method {} {}", method_name, std::str::from_utf8(&msg.body).unwrap());
    ///             0
    ///         })
    ///         .await;
    /// }
    /// ```
    #[cfg(feature = "direct-methods")]
    pub async fn on_direct_method<T>(&mut self, handler: T)
    where
        T: Fn(String, Message) -> i32 + Send + 'static,
    {
        self.transport
            .set_message_handler(
                &self.device_id,
                MessageHandler::DirectMethod(Box::new(handler)),
            )
            .await;
    }

    /// Define the cloud to device message handler
    ///
    /// # Example
    /// ```no_run
    /// use azure_iot_sdk::client::IoTHubClient;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let mut client = IoTHubClient::with_device_key(
    ///         "iothubname.azure-devices.net".into(),
    ///         "MyDeviceId".into(),
    ///         "TheAccessKey".into()).await;
    ///
    ///     client
    ///        .on_twin_update(|msg| {
    ///            println!("Received message {:?}", msg);
    ///        })
    ///        .await;
    /// }
    /// ```
    #[cfg(feature = "twin-properties")]
    pub async fn on_twin_update<T>(&mut self, handler: T)
    where
        T: Fn(Message) + Send + 'static,
    {
        self.transport
            .set_message_handler(
                &self.device_id,
                MessageHandler::TwinUpdate(Box::new(handler)),
            )
            .await;
    }
}

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(generate_sas("myiothub.azure-devices.net", "FirstDevice", "O+H9VTcdJP0Tqkl7bh4nVG0OJNrAataMpuWB54D0VEc=", 1_587_123_309), "SharedAccessSignature sr=myiothub.azure-devices.net%2Fdevices%2FFirstDevice&sig=vn0%2BgyIUKgaBhEU0ypyOhJ0gPK5fSY1TKdvcJ1HxhnQ%3D&se=1587123309".to_string());
    }

    #[test]
    fn test_provision_sas() {
        assert_eq!(generate_registration_sas("0ne000EEBBD", "FirstDevice", "O+H9VTcdJP0Tqkl7bh4nVG0OJNrAataMpuWB54D0VEc=", 1_591_921_306), "SharedAccessSignature sr=0ne000EEBBD%2fregistrations%2fFirstDevice&sig=hwgBlMB6G2Zg5ZcYtwmtLVKRbifiSCPfUMyscbVWa8o%3D&se=1591921306".to_string());
    }

    #[test]
    fn test_mqtt_connect() {}
}
