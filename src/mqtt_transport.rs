use crate::message::{DirectMethodResponse, Message, SendType};

use async_trait::async_trait;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::time;
use tokio_tls::{TlsConnector, TlsStream};

use mqtt::control::variable_header::ConnectReturnCode;
use mqtt::packet::*;
use mqtt::{Encodable, QualityOfService};
use mqtt::{TopicFilter, TopicName};

const KEEP_ALIVE: u16 = 10;
const DIRECT_METHOD_TOPIC_PREFIX: &str = "$iothub/methods/POST/";
const TWIN_TOPIC_PREFIX: &str = "$iothub/twin/";
const REQUEST_ID_PARAM: &str = "?$rid=";

fn receive_topic_prefix(device_id: &str) -> String {
    format!("devices/{}/messages/devicebound/", device_id)
}

#[async_trait]
pub(crate) trait Transport {
    async fn new(hub_name: String, device_id: String, sas: String) -> Self;
    async fn send_message(&mut self, message: Message);
    async fn set_message_handler(&mut self, device_id: &str, handler: MessageHandler);
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
/// ```no_run
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
            SendType::Subscribe(topic_filters) => {
                let subscribe_packet = SubscribePacket::new(10, topic_filters);
                let mut buf = Vec::new();
                subscribe_packet.encode(&mut buf).unwrap();
                write_socket.write_all(&buf[..]).await.unwrap();
            }
        }
    }
}

pub(crate) enum MessageHandler {
    Message(Box<dyn Fn(Message) + Send>),
    TwinUpdate(Box<dyn Fn(Message) + Send>),
    DirectMethod(Box<dyn Fn(String, Message) -> i32 + Send>),
}

/// Start receive async loop as task that will send received messages from the cloud onto mpsc
///
/// Arguments
///
/// * `read_socket` -
/// * `device_id` -
/// * `sender` -
async fn receive<S>(
    mut read_socket: S,
    device_id: String,
    mut sender: Sender<SendType>,
    mut receiver: Receiver<MessageHandler>,
) where
    S: AsyncReadExt + Unpin,
{
    let rx_topic_prefix = receive_topic_prefix(&device_id);
    let mut message_handler: Option<Box<dyn Fn(Message) + Send>> = None;
    let mut twin_handler: Option<Box<dyn Fn(Message) + Send>> = None;
    let mut method_handler: Option<Box<dyn Fn(String, Message) -> i32 + Send>> = None;

    loop {
        tokio::select! {
            Some(handler) = receiver.recv() => {
              match handler {
                MessageHandler::Message(msg_handler) => message_handler = Some(msg_handler),
                MessageHandler::TwinUpdate(msg_handler) => twin_handler = Some(msg_handler),
                MessageHandler::DirectMethod(msg_handler) => method_handler = Some(msg_handler),
              }
            }
            Ok(packet) = VariablePacket::parse(&mut read_socket) => {
              trace!("PACKET {:?}", packet);
              match packet {
                VariablePacket::PingrespPacket(..) => {
                    info!("Receiving PINGRESP from broker ..");
                }
                VariablePacket::PublishPacket(ref publ) => {
                    let mut message = Message::new(publ.payload_ref()[..].to_vec());
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
                        if let Some(msg_handler) = &message_handler {
                          msg_handler(message);
                        }
                        // sender.send(MessageType::C2DMessage(message)).await.unwrap();
                    } else if publ.topic_name().starts_with(TWIN_TOPIC_PREFIX) {
                      if let Some(twin_handler) = &twin_handler {
                        twin_handler(message);
                      }
                    } else if publ.topic_name().starts_with(DIRECT_METHOD_TOPIC_PREFIX) {
                        // Sent to topic in format $iothub/methods/POST/{method name}/?$rid={request id}

                        // Strip the prefix from the topic left with {method name}/$rid={request id}
                        let details = &publ.topic_name()[DIRECT_METHOD_TOPIC_PREFIX.len()..];

                        let method_components: Vec<_> = details.split('/').collect();

                        // If we don't have a handler respond with -1
                        let mut method_response = -1;

                        if let Some(method_handler) = &method_handler {
                          method_response = method_handler(method_components[0].to_string(), message);
                        }

                        let request_id = method_components[1][REQUEST_ID_PARAM.len()..].to_string();
                        let to_respond_with = SendType::RespondToDirectMethod(DirectMethodResponse::new(request_id, method_response, None));
                        sender.send(to_respond_with).await.unwrap();
                    }
                }
                _ => {}
              }
            }
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

#[derive(Debug, Clone)]
pub(crate) struct MqttTransport {
    pub(crate) handler_tx: Sender<MessageHandler>,
    pub(crate) d2c_sender: Sender<SendType>,
}

#[async_trait]
impl Transport for MqttTransport {
    async fn new(hub_name: String, device_id: String, sas: String) -> Self {
        let mut socket = tcp_connect(&hub_name).await;

        let mut conn = ConnectPacket::new("MQTT", &device_id);
        conn.set_client_identifier(&device_id);
        conn.set_clean_session(false);
        conn.set_keep_alive(KEEP_ALIVE);
        conn.set_user_name(Some(format!(
            "{}/{}/?api-version=2018-06-30",
            hub_name, device_id
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

        let (read_socket, write_socket) = tokio::io::split(socket);

        let (mut d2c_sender, d2c_receiver) = channel::<SendType>(100);
        tokio::spawn(create_sender(
            write_socket,
            device_id.to_owned(),
            d2c_receiver,
        ));

        tokio::spawn(ping(KEEP_ALIVE / 2, d2c_sender.clone()));

        let (handler_tx, handler_rx) = channel::<MessageHandler>(3);
        if cfg!(feature = "direct-methods")
            || cfg!(feature = "twin-properties")
            || cfg!(feature = "c2d-messages")
        {
            let sender = d2c_sender.clone();
            tokio::spawn(receive(
                read_socket,
                device_id.to_owned(),
                sender,
                handler_rx,
            ));
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
            handler_tx,
            d2c_sender,
        }
    }

    async fn send_message(&mut self, message: Message) {
        self.d2c_sender
            .send(SendType::Message(message))
            .await
            .unwrap();
    }

    async fn set_message_handler(&mut self, device_id: &str, handler: MessageHandler) {
        // TODO: figure out how to do self.handler_tx.send and then match on enum type after for subscription
        // to reduce duplication here
        match handler {
            MessageHandler::Message(_) => {
                if self.handler_tx.send(handler).await.is_err() {
                    error!("Failed to set message handler");
                    return;
                }
                self.subscribe_to_c2d_messages(device_id).await
            }
            MessageHandler::DirectMethod(_) => {
                if self.handler_tx.send(handler).await.is_err() {
                    error!("Failed to set message handler");
                    return;
                }
                self.subscribe_to_direct_methods().await
            }
            MessageHandler::TwinUpdate(_) => {
                if self.handler_tx.send(handler).await.is_err() {
                    error!("Failed to set message handler");
                    return;
                }
                self.subscribe_to_twin_updates().await
            }
        }
    }
}

impl MqttTransport {
    #[cfg(feature = "c2d-messages")]
    async fn subscribe_to_c2d_messages(&mut self, device_id: &str) {
        self.d2c_sender
            .send(SendType::Subscribe(vec![(
                TopicFilter::new(format!("{}#", receive_topic_prefix(device_id))).unwrap(),
                QualityOfService::Level0,
            )]))
            .await
            .unwrap();
    }

    #[cfg(feature = "twin-properties")]
    async fn subscribe_to_twin_updates(&mut self) {
        let topics = vec![
            (
                TopicFilter::new(format!("{}res/#", TWIN_TOPIC_PREFIX)).unwrap(),
                QualityOfService::Level0,
            ),
            (
                TopicFilter::new(format!("{}PATCH/properties/desired/#", TWIN_TOPIC_PREFIX))
                    .unwrap(),
                QualityOfService::Level0,
            ),
        ];
        self.d2c_sender
            .send(SendType::Subscribe(topics))
            .await
            .unwrap();
    }

    #[cfg(feature = "direct-methods")]
    async fn subscribe_to_direct_methods(&mut self) {
        self.d2c_sender
            .send(SendType::Subscribe(vec![(
                TopicFilter::new(format!("{}#", DIRECT_METHOD_TOPIC_PREFIX)).unwrap(),
                QualityOfService::Level0,
            )]))
            .await
            .unwrap();
    }
}
