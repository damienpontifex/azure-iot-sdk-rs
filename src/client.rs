use crate::message::{DirectMethodInvokation, Message, MessageType, SendType};

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

fn type_of<T>(_: T) -> &'static str {
    std::any::type_name::<T>()
}

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
    d2c_sender: Sender<SendType>,
    c2d_receiver: Option<Receiver<MessageType>>,
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
    /// ```
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
    /// ```
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
    /// ```
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
        let mut socket = tcp_connect(&hub_name).await;

        mqtt_connect(&hub_name, &device_id, &sas, &mut socket).await;

        let (read_socket, mut write_socket) = tokio::io::split(socket);

        if cfg!(feature = "direct-methods")
            || cfg!(feature = "twin-properties")
            || cfg!(feature = "c2d-messages")
        {
            // Only send subscriptions and start receiver tasks if client is interested
            // in subscribing to any of these capabilities

            subscribe(&mut write_socket, &device_id).await;
        } else {
            debug!("Skipping topic subscribe as no features enabled");
        }

        let (mut d2c_sender, d2c_receiver) = channel::<SendType>(100);
        tokio::spawn(create_sender(
            write_socket,
            device_id.to_owned(),
            d2c_receiver,
        ));

        tokio::spawn(ping(KEEP_ALIVE / 2, d2c_sender.clone()));

        let mut c2d_receiver: Option<Receiver<MessageType>> = None;
        if cfg!(feature = "direct-methods")
            || cfg!(feature = "twin-properties")
            || cfg!(feature = "c2d-messages")
        {
            let (tx, rx) = channel::<MessageType>(100);
            tokio::spawn(receive(read_socket, device_id.to_owned(), tx));
            c2d_receiver = Some(rx);
        }

        #[cfg(feature = "twin-properties")]
        {
            // Send empty message so hub will respond with device twin data
            d2c_sender
                .send(SendType::RequestTwinProperties("0".into()))
                .await
                .unwrap();
        }

        Self {
            device_id,
            d2c_sender,
            c2d_receiver,
        }
    }

    /// Send a device to cloud message for this device to the IoT Hub
    ///
    /// #Example
    /// ```
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
        self.d2c_sender
            .send(SendType::Message(message))
            .await
            .unwrap();
    }

    /// Gets a new Sender channel that is paired with the hub device to
    /// cloud send functionality
    ///
    /// #Example
    /// ```
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
    pub fn sender(&mut self) -> Sender<SendType> {
        self.d2c_sender.clone()
    }

    /// Get the receiver channel where cloud to device messages will be sent to.
    /// This can only be retrieved once as the receive side of the channel is a to
    /// one operation. If the receiver has been retrieved already, `None` will be returned
    ///
    /// # Example
    /// ```
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
        if self.c2d_receiver.is_some() {
            Some(std::mem::replace(&mut self.c2d_receiver, None).unwrap())
        } else {
            None
        }
    }
}

async fn ping(interval: u16, mut sender: Sender<SendType>) {
    let mut ping_interval = time::interval(time::Duration::from_secs(interval.into()));
    loop {
        ping_interval.tick().await;

        sender.send(SendType::Ping).await.unwrap();
    }
}

/// Connect to Azure IoT Hub
///
/// # Arguments
///
/// * `hub_name` - The IoT hub resource name
/// * `device_id` - The registered device to connect as
/// * `sas` - The shared access signature for the device to authenticate with
///
/// # Example
/// ```
/// // let (read_socket, write_socket) = client::connect("myiothub".to_string(), "myfirstdevice".to_string(), "SharedAccessSignature sr=myiothub.azure-devices.net%2Fdevices%2Fmyfirstdevice&sig=blahblah&se=1586909077".to_string()).await;
/// ```
async fn tcp_connect(iot_hub: &str) -> TlsStream<TcpStream> {
    let socket = TcpStream::connect((iot_hub, 8883)).await.unwrap();

    trace!("Connected to tcp socket {:?}", socket);

    let cx = TlsConnector::from(
        native_tls::TlsConnector::builder()
            .min_protocol_version(Some(native_tls::Protocol::Tlsv12))
            .build()
            .unwrap(),
    );

    let socket = cx.connect(&iot_hub, socket).await.unwrap();

    trace!("Connected tls context {:?}", cx);

    socket
}

async fn mqtt_connect<S>(iot_hub: &str, device_id: &str, sas: &str, mut socket: S)
where
    S: AsyncWriteExt + AsyncRead + Unpin,
{
    let mut conn = ConnectPacket::new("MQTT", device_id);
    conn.set_client_identifier(device_id);
    conn.set_clean_session(false);
    conn.set_keep_alive(KEEP_ALIVE);
    conn.set_user_name(Some(format!(
        "{}/{}/?api-version=2018-06-30",
        iot_hub, device_id
    )));
    conn.set_password(Some(sas.to_string()));

    let mut buf = Vec::new();
    conn.encode(&mut buf).unwrap();
    socket.write_all(&buf[..]).await.unwrap();

    let packet = VariablePacket::parse(&mut socket).await;

    trace!("PACKET {:?}", packet);
    match packet {
        Ok(VariablePacket::ConnackPacket(connack)) => {
            if connack.connect_return_code() != ConnectReturnCode::ConnectionAccepted {
                panic!(
                    "Failed to connect to server, return code {:?}",
                    connack.connect_return_code()
                );
            }
        }
        Ok(pck) => panic!("Unexpected packet received after connect {:?}", pck),
        Err(err) => panic!("Error decoding connack packet {:?}", err),
    }
}

fn receive_topic_prefix(device_id: &str) -> String {
    format!("devices/{}/messages/devicebound/", device_id)
}

/// Async loop to create task that will received from mpsc receiver and send to IoT hub
///
/// # Arguments
///
/// * `write_socket` -
/// * `device_id` -
/// * `receiver` -
async fn create_sender<S>(mut write_socket: S, device_id: String, mut receiver: Receiver<SendType>)
where
    S: AsyncWriteExt + Unpin,
{
    let send_topic = TopicName::new(format!("devices/{}/messages/events/", device_id)).unwrap();
    while let Some(sent) = receiver.recv().await {
        match sent {
            SendType::Message(message) => {
                // TODO: Append properties and system properties to topic path
                trace!("Sending message {:?}", message);
                let publish_packet = PublishPacket::new(
                    send_topic.clone(),
                    QoSWithPacketIdentifier::Level0,
                    message.body,
                );
                let mut buf = Vec::new();
                publish_packet.encode(&mut buf).unwrap();
                write_socket.write_all(&buf[..]).await.unwrap();
            }
            SendType::Ping => {
                info!("Sending PINGREQ to broker");

                let pingreq_packet = PingreqPacket::new();

                let mut buf = Vec::new();
                pingreq_packet.encode(&mut buf).unwrap();
                write_socket.write_all(&buf).await.unwrap();
            }
            SendType::RespondToDirectMethod(response) => {
                // TODO: Append properties and system properties to topic path
                trace!(
                    "Responding to direct method with rid = {}",
                    response.request_id
                );
                let publish_packet = PublishPacket::new(
                    TopicName::new(format!(
                        "$iothub/methods/res/{}/?$rid={}",
                        response.status, response.request_id
                    ))
                    .unwrap(),
                    QoSWithPacketIdentifier::Level0,
                    response.body,
                );
                let mut buf = Vec::new();
                publish_packet.encode(&mut buf).unwrap();
                write_socket.write_all(&buf[..]).await.unwrap();
            }
            SendType::RequestTwinProperties(request_id) => {
                trace!(
                    "Requesting device twin properties with rid = {}",
                    request_id
                );
                let packet = PublishPacket::new(
                    TopicName::new(format!("{}GET/?$rid={}", TWIN_TOPIC_PREFIX, request_id))
                        .unwrap(),
                    QoSWithPacketIdentifier::Level0,
                    "".as_bytes(),
                );
                let mut buf = vec![];
                packet.encode(&mut buf).unwrap();
                write_socket.write_all(&buf[..]).await.unwrap();
            }
        }
    }
}

async fn subscribe<S>(write_socket: &mut S, device_id: &str)
where
    S: AsyncWriteExt + Unpin,
{
    let mut topics = vec![];

    #[cfg(feature = "c2d-messages")]
    {
        topics.push((
            TopicFilter::new(format!("{}#", receive_topic_prefix(device_id))).unwrap(),
            QualityOfService::Level0,
        ));
    }

    #[cfg(feature = "twin-properties")]
    {
        topics.extend_from_slice(&[
            (
                TopicFilter::new(format!("{}res/#", TWIN_TOPIC_PREFIX)).unwrap(),
                QualityOfService::Level0,
            ),
            (
                TopicFilter::new(format!("{}PATCH/properties/desired/#", TWIN_TOPIC_PREFIX))
                    .unwrap(),
                QualityOfService::Level0,
            ),
        ]);
    }

    #[cfg(feature = "direct-methods")]
    {
        topics.push((
            TopicFilter::new(format!("{}#", DIRECT_METHOD_TOPIC_PREFIX)).unwrap(),
            QualityOfService::Level0,
        ));
    }

    if !topics.is_empty() {
        debug!("Subscribing to topics {:?}", topics);

        let subscribe_packet = SubscribePacket::new(10, topics);
        let mut buf = Vec::new();
        subscribe_packet.encode(&mut buf).unwrap();
        write_socket.write_all(&buf[..]).await.unwrap();
    }
}

/// Start receive async loop as task that will send received messages from the cloud onto mpsc
///
/// Arguments
///
/// * `read_socket` -
/// * `device_id` -
/// * `sender` -
async fn receive<S>(mut read_socket: S, device_id: String, mut sender: Sender<MessageType>)
where
    S: AsyncReadExt + Unpin,
{
    let rx_topic_prefix = receive_topic_prefix(&device_id);
    loop {
        let packet = match VariablePacket::parse(&mut read_socket).await {
            Ok(pk) => pk,
            Err(VariablePacketError::IoError(err)) => {
                error!("IO {:?}", err);
                continue;
            }
            Err(err) => {
                error!("Error in receiving packet {}, {}", err, type_of(&err));
                continue;
            }
        };
        trace!("PACKET {:?}", packet);

        match packet {
            VariablePacket::PingrespPacket(..) => {
                info!("Receiving PINGRESP from broker ..");
            }
            VariablePacket::PublishPacket(ref publ) => {
                let msg = match std::str::from_utf8(&publ.payload_ref()[..]) {
                    Ok(msg) => msg,
                    Err(err) => {
                        error!("Failed to decode publish message {:?}", err);
                        continue;
                    }
                };
                let mut message = Message::new(msg.to_owned());
                info!("PUBLISH ({}): {:?}", publ.topic_name(), message);

                if publ.topic_name().starts_with(&rx_topic_prefix) {
                    let properties = publ.topic_name().trim_start_matches(&rx_topic_prefix);
                    let property_tuples =
                        serde_urlencoded::from_str::<Vec<(String, String)>>(properties).unwrap();
                    for (key, value) in property_tuples {
                        // We have properties after the topic path
                        if key.starts_with("$.") {
                            message
                                .system_properties
                                .insert(key[2..].to_string(), value);
                        } else {
                            message.properties.insert(key, value);
                        }
                    }
                    sender.send(MessageType::C2DMessage(message)).await.unwrap();
                } else if publ.topic_name().starts_with(TWIN_TOPIC_PREFIX) {
                    sender
                        .send(MessageType::DesiredPropertyUpdate(message))
                        .await
                        .unwrap();
                } else if publ.topic_name().starts_with(DIRECT_METHOD_TOPIC_PREFIX) {
                    // Sent to topic in format $iothub/methods/POST/{method name}/?$rid={request id}

                    // Strip the prefix from the topic left with {method name}/$rid={request id}
                    let details = &publ.topic_name()[DIRECT_METHOD_TOPIC_PREFIX.len()..];

                    let method_components: Vec<_> = details.split('/').collect();

                    sender
                        .send(MessageType::DirectMethod(DirectMethodInvokation {
                            method_name: method_components[0].to_string(),
                            message,
                            request_id: method_components[1][REQUEST_ID_PARAM.len()..].to_string(),
                        }))
                        .await
                        .unwrap();

                    // TODO: provide tokio::sync::oneshot to respond to direct method on
                }
            }
            _ => {}
        }
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
