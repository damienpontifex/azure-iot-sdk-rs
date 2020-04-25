use crate::message::{DirectMethodInvokation, Message, MessageType, SendType};
use crate::mqtt_transport::MqttTransport;

use chrono::{Duration, Utc};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::time;
use tokio_tls::{TlsConnector, TlsStream};

use mqtt::control::variable_header::ConnectReturnCode;
use mqtt::packet::*;
use mqtt::{Encodable, QualityOfService};
use mqtt::{TopicFilter, TopicName};

use hmac::{Hmac, Mac};
use sha2::Sha256;

const DEVICEID_KEY: &str = "DeviceId";
const HOSTNAME_KEY: &str = "HostName";
const SHAREDACCESSKEY_KEY: &str = "SharedAccessKey";
const KEEP_ALIVE: u16 = 10;

const DIRECT_METHOD_TOPIC_PREFIX: &str = "$iothub/methods/POST/";
const REQUEST_ID_PARAM: &str = "$rid=";
const TWIN_TOPIC_PREFIX: &str = "$iothub/twin/";

#[derive(Debug)]
pub struct IoTHubClient {
    device_id: String,
    transport: MqttTransport,
    // d2c_sender: Sender<SendType>,
    // c2d_receiver: Option<Receiver<MessageType>>,
}

fn generate_sas(hub: &str, device_id: &str, key: &str, expiry_timestamp: i64) -> String {
    let resource_uri = format!("{}/devices/{}", hub, device_id);

    const FRAGMENT: &percent_encoding::AsciiSet = &percent_encoding::CONTROLS.add(b'/');

    let resource_uri = percent_encoding::utf8_percent_encode(&resource_uri, FRAGMENT);
    let to_sign = format!("{}\n{}", &resource_uri, expiry_timestamp);

    let key = base64::decode(&key).unwrap();
    let mut mac = Hmac::<Sha256>::new_varkey(&key).unwrap();
    mac.input(to_sign.as_bytes());
    let mac_result = mac.result().code();
    let signature = base64::encode(mac_result.as_ref());

    let pairs = &vec![("sig", signature)];
    let token = serde_urlencoded::to_string(pairs).unwrap();

    let sas = format!(
        "SharedAccessSignature sr={}&{}&se={}",
        resource_uri, token, expiry_timestamp
    );

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
        let transport = MqttTransport::new(hub_name, device_id.clone(), sas).await;

        Self {
            device_id,
            transport,
        }
    }

    /// Send a device to cloud message for this device to the IoT Hub
    ///
    /// #Example
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
    ///             .set_body_from(format!("Message #{}", count))
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

    /// Gets a new Sender channel that is paired with the hub device to
    /// cloud send functionality
    ///
    /// #Example
    /// ```no_run
    /// use azure_iot_sdk::client::IoTHubClient;
    /// use azure_iot_sdk::message::{Message, SendType};
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
    ///     let mut tx = client.sender();
    ///
    ///     loop {
    ///         interval.tick().await;
    ///
    ///         let msg = Message::builder()
    ///             .set_body_from(format!("Message #{}", count))
    ///             .set_message_id(format!("{}-t", count))
    ///             .build();
    ///
    ///         tx.send(SendType::Message(msg))
    ///             .await
    ///             .unwrap();
    ///
    ///         count += 1;
    ///     }
    /// }
    /// ```
    pub fn sender(&self) -> Sender<SendType> {
        self.transport.d2c_sender.clone()
    }

    /// Get the receiver channel where cloud to device messages will be sent to.
    /// This can only be retrieved once as the receive side of the channel is a to
    /// one operation. If the receiver has been retrieved already, `None` will be returned
    ///
    /// # Example
    /// ```no_run
    /// use azure_iot_sdk::client::IoTHubClient;
    /// use azure_iot_sdk::message::MessageType;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let mut client = IoTHubClient::with_device_key(
    ///         "iothubname.azure-devices.net".into(),
    ///         "MyDeviceId".into(),
    ///         "TheAccessKey".into()).await;
    ///
    ///     let mut rx = client.get_receiver().unwrap();
    ///     while let Some(msg) = rx.recv().await {
    ///         match msg {
    ///             MessageType::C2DMessage(message) => println!("Received message {:?}", message),
    ///             MessageType::DirectMethod(direct_method) => println!("Received direct method {:?}", direct_method),
    ///             MessageType::DesiredPropertyUpdate(property_update) => println!("Received property update {:?}", property_update)
    ///         }
    ///     }
    /// }
    /// ```

    pub fn get_receiver(&mut self) -> Option<Receiver<MessageType>> {
        if self.transport.c2d_receiver.is_some() {
            Some(std::mem::replace(&mut self.transport.c2d_receiver, None).unwrap())
        } else {
            None
        }
    }

    #[cfg(feature = "c2d-message")]
    pub fn on_message<T>(&self, handler: T)
    where
        T: Fn(Message),
    {
    }

    #[cfg(feature = "direct-methods")]
    pub fn on_direct_method<T>(&self, handler: T)
    where
        T: Fn(String, Message) -> i32,
    {
    }

    #[cfg(feature = "twin-properties")]
    pub fn on_twin_update<T>(&self, handler: T)
    where
        T: Fn(Message),
    {
    }
}

async fn ping(interval: u16, mut sender: Sender<SendType>) {
    let mut ping_interval = time::interval(time::Duration::from_secs(interval.into()));
    loop {
        ping_interval.tick().await;

        sender.send(SendType::Ping).await.unwrap();
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
    fn test_mqtt_connect() {}
}
