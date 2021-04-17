use mqtt::control::variable_header::ConnectReturnCode;
use mqtt::packet::*;
use mqtt::topic_name::TopicNameError;
use mqtt::Encodable;
use mqtt::TopicName;
#[cfg(any(
    feature = "direct-methods",
    feature = "c2d-messages",
    feature = "twin-properties"
))]
use mqtt::{QualityOfService, TopicFilter};
use tokio::io::AsyncWriteExt;
use tokio::io::{ReadHalf, WriteHalf};
use tokio::net::TcpStream;
#[cfg(any(
    feature = "direct-methods",
    feature = "c2d-messages",
    feature = "twin-properties"
))]
use tokio::sync::mpsc::{channel, Receiver};
use tokio::sync::Mutex;
use tokio_native_tls::{TlsConnector, TlsStream};

use async_trait::async_trait;

use crate::message::Message;
#[cfg(any(
    feature = "direct-methods",
    feature = "c2d-messages",
    feature = "twin-properties"
))]
use crate::message::MessageType;
#[cfg(feature = "direct-methods")]
use crate::message::{DirectMethodInvocation, DirectMethodResponse};
use crate::{token::TokenSource, transport::Transport};
use chrono::{Duration, Utc};
// use futures::future::{AbortHandle, Abortable};
use std::sync::Arc;

// Incoming topic names
#[cfg(feature = "direct-methods")]
const METHOD_POST_TOPIC_FILTER: &str = "$iothub/methods/POST/#";
#[cfg(feature = "direct-methods")]
const METHOD_POST_TOPIC_PREFIX: &str = "$iothub/methods/POST/";
#[cfg(feature = "twin-properties")]
const TWIN_RESPONSE_TOPIC_FILTER: &str = "$iothub/twin/res/#";
// const TWIN_RESPONSE_TOPIC_PREFIX: &str = "$iothub/twin/res/";
#[cfg(feature = "twin-properties")]
const TWIN_PATCH_TOPIC_FILTER: &str = "$iothub/twin/PATCH/properties/desired/#";
#[cfg(feature = "twin-properties")]
const TWIN_PATCH_TOPIC_PREFIX: &str = "$iothub/twin/PATCH/properties/desired/";
#[cfg(feature = "twin-properties")]
const TWIN_PATCH_UPDATE_PREFIX: &str = "$iothub/twin/PATCH/properties/reported/";

// Outgoing topic names
#[cfg(feature = "direct-methods")]
fn method_response_topic(status: i32, request_id: &str) -> String {
    format!("$iothub/methods/res/{}/?$rid={}", status, request_id)
}

#[cfg(feature = "twin-properties")]
fn twin_get_topic(request_id: &str) -> String {
    format!("$iothub/twin/GET/?$rid={}", request_id)
}

#[cfg(feature = "twin-properties")]
fn twin_update_topic(request_id: &str) -> String {
    format!("{}?$rid={}", TWIN_PATCH_UPDATE_PREFIX, request_id)
}

#[cfg(feature = "c2d-messages")]
fn device_bound_messages_topic_filter(device_id: &str) -> String {
    format!("devices/{}/messages/devicebound/#", device_id)
}
#[cfg(feature = "c2d-messages")]
fn device_bound_messages_topic_prefix(device_id: &str) -> String {
    format!("devices/{}/messages/devicebound/", device_id)
}
fn cloud_bound_messages_topic(device_id: &str) -> String {
    format!("devices/{}/messages/events/", device_id)
}

const KEEP_ALIVE: u16 = 10;
#[cfg(feature = "direct-methods")]
const REQUEST_ID_PARAM: &str = "?$rid=";

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
async fn tcp_connect(iot_hub: &str) -> crate::Result<TlsStream<TcpStream>> {
    let socket = TcpStream::connect((iot_hub, 8883)).await?;

    trace!("Connected to tcp socket {:?}", socket);

    let cx = TlsConnector::from(
        native_tls::TlsConnector::builder()
            .min_protocol_version(Some(native_tls::Protocol::Tlsv12))
            .build()
            .unwrap(),
    );

    let socket = cx.connect(&iot_hub, socket).await?;

    trace!("Connected tls context {:?}", cx);

    Ok(socket)
}

pub(crate) async fn mqtt_connect(
    iot_hub: &str,
    device_id: &str,
    username: impl ToString,
    password: impl ToString,
) -> crate::Result<TlsStream<TcpStream>> {
    let mut socket = tcp_connect(iot_hub).await?;

    let mut conn = ConnectPacket::new(device_id);
    conn.set_client_identifier(device_id);
    conn.set_clean_session(false);
    conn.set_keep_alive(KEEP_ALIVE);
    conn.set_user_name(Some(username.to_string()));
    conn.set_password(Some(password.to_string()));

    let mut buf = Vec::new();
    conn.encode(&mut buf).unwrap();
    socket.write_all(&buf[..]).await?;

    let packet = VariablePacket::parse(&mut socket).await;

    trace!("PACKET {:?}", packet);
    match packet {
        //TODO: Enum error type instead of strings
        Ok(VariablePacket::ConnackPacket(connack)) => {
            if connack.connect_return_code() != ConnectReturnCode::ConnectionAccepted {
                Err(format!(
                    "Failed to connect to server, return code {:?}",
                    connack.connect_return_code()
                ))
            } else {
                Ok(())
            }
        }
        Ok(pck) => Err(format!(
            "Unexpected packet received after connect {:?}",
            pck
        )),
        Err(err) => Err(format!("Error decoding connack packet {:?}", err)),
    }?;

    Ok(socket)
}

// async fn ping(interval: u16) {
//     let mut ping_interval = time::interval(time::Duration::from_secs(interval.into()));
//     loop {
//         ping_interval.tick().await;

//         // sender.send(SendType::Ping).await.unwrap();
//     }
// }

///
#[derive(Debug, Clone)]
pub(crate) struct MqttTransport {
    write_socket: Arc<Mutex<WriteHalf<TlsStream<TcpStream>>>>,
    read_socket: Arc<Mutex<ReadHalf<TlsStream<TcpStream>>>>,
    d2c_topic: TopicName,
    device_id: String,
    #[cfg(feature = "c2d-messages")]
    rx_topic_prefix: String,
    // rx_loop_handle: Option<AbortHandle>,
}

// impl Drop for MqttTransport {
//     fn drop(&mut self) {
//         if let Some(recv_abort_handle) = &self.rx_loop_handle {
//             recv_abort_handle.abort();
//         }
//     }
// }

#[async_trait]
impl Transport<MqttTransport> for MqttTransport {
    async fn new<TS>(
        hub_name: &str,
        device_id: String,
        token_source: TS,
    ) -> crate::Result<MqttTransport>
    where
        TS: TokenSource + Sync + Send,
    {
        let user_name = format!("{}/{}/?api-version=2018-06-30", hub_name, device_id);

        let expiry = Utc::now() + Duration::days(1);
        trace!("Generating token that will expire at {}", expiry);
        let token = token_source.get(&expiry);
        trace!("Using token {}", token);

        let socket = mqtt_connect(&hub_name, &device_id, user_name, token).await?;

        let (read_socket, write_socket) = tokio::io::split(socket);

        Ok(Self {
            write_socket: Arc::new(Mutex::new(write_socket)),
            read_socket: Arc::new(Mutex::new(read_socket)),
            d2c_topic: TopicName::new(cloud_bound_messages_topic(&device_id)).unwrap(),
            device_id: device_id.to_string(),
            #[cfg(feature = "c2d-messages")]
            rx_topic_prefix: device_bound_messages_topic_prefix(&device_id),
            // rx_loop_handle: None,
        })
    }

    async fn send_message(&mut self, message: Message) -> crate::Result<()> {
        let full_topic = build_topic_name(&self.d2c_topic, &message).unwrap();
        trace!("Sending message {:?} to topic {:?}", message, full_topic);
        let publish_packet =
            PublishPacket::new(full_topic, QoSWithPacketIdentifier::Level0, message.body);
        let mut buf = Vec::new();
        publish_packet.encode(&mut buf).unwrap();

        self.write_socket
            .lock()
            .await
            .write_all(&buf[..])
            .await
            .map_err(|e| e.into())
    }

    #[cfg(feature = "twin-properties")]
    async fn send_property_update(&mut self, request_id: &str, body: &str) -> crate::Result<()> {
        trace!("Publishing twin properties with rid = {}", request_id);
        let packet = PublishPacket::new(
            TopicName::new(twin_update_topic(&request_id)).unwrap(),
            QoSWithPacketIdentifier::Level0,
            body.as_bytes(),
        );
        let mut buf = vec![];
        packet.encode(&mut buf).unwrap();
        self.write_socket
            .lock()
            .await
            .write_all(&buf[..])
            .await
            .map_err(|e| e.into())
    }

    #[cfg(feature = "twin-properties")]
    async fn request_twin_properties(&mut self, request_id: &str) -> crate::Result<()> {
        trace!(
            "Requesting device twin properties with rid = {}",
            request_id
        );
        let packet = PublishPacket::new(
            TopicName::new(twin_get_topic(&request_id)).unwrap(),
            QoSWithPacketIdentifier::Level0,
            "".as_bytes(),
        );
        let mut buf = vec![];
        packet.encode(&mut buf).unwrap();
        self.write_socket
            .lock()
            .await
            .write_all(&buf[..])
            .await
            .map_err(|e| e.into())
    }

    #[cfg(feature = "direct-methods")]
    async fn respond_to_direct_method(
        &mut self,
        response: DirectMethodResponse,
    ) -> crate::Result<()> {
        // TODO: Append properties and system properties to topic path
        trace!(
            "Responding to direct method with rid = {}",
            response.request_id
        );
        let publish_packet = PublishPacket::new(
            TopicName::new(method_response_topic(response.status, &response.request_id)).unwrap(),
            QoSWithPacketIdentifier::Level0,
            response.body,
        );
        let mut buf = Vec::new();
        publish_packet.encode(&mut buf).unwrap();
        self.write_socket
            .lock()
            .await
            .write_all(&buf[..])
            .await
            .map_err(|e| e.into())
    }

    async fn ping(&mut self) -> crate::Result<()> {
        info!("Sending PINGREQ to broker");

        let pingreq_packet = PingreqPacket::new();

        let mut buf = Vec::new();
        pingreq_packet.encode(&mut buf).unwrap();
        self.write_socket
            .lock()
            .await
            .write_all(&buf)
            .await
            .map_err(|e| e.into())
    }

    #[cfg(any(
        feature = "direct-methods",
        feature = "c2d-messages",
        feature = "twin-properties",
        feature = "error-handling-messages",
    ))]
    async fn get_receiver(&mut self) -> Receiver<MessageType> {
        let (handler_tx, handler_rx) = channel::<MessageType>(3);

        let mut cloned_self = self.clone();
        let _ = tokio::spawn(async move {
            loop {
                let mut socket = cloned_self.read_socket.lock().await;
                let packet = match VariablePacket::parse(&mut *socket).await {
                    Ok(pk) => pk,
                    Err(err) => {
                        #[cfg(feature = "error-handling-messages")]
                        if handler_tx
                            .send(MessageType::ErrorReceive(err))
                            .await
                            .is_err()
                        {
                            break;
                        }
                        #[cfg(not(feature = "error-handling-messages"))]
                        error!("Error in receiving packet {}", err);
                        continue;
                    }
                };

                // Networking
                trace!("Received PACKET {:?}", packet);
                match packet {
                    // TODO: handle ping req from server and we should send ping response in return
                    VariablePacket::PingrespPacket(..) => {
                        info!("Receiving PINGRESP from broker ..");
                    }
                    VariablePacket::PublishPacket(ref publ) => {
                        let mut message = Message::new(publ.payload_ref()[..].to_vec());
                        trace!("PUBLISH ({}): {:?}", publ.topic_name(), message);

                        #[cfg(feature = "c2d-messages")]
                        if publ.topic_name().starts_with(&cloned_self.rx_topic_prefix) {
                            // C2D Message
                            let properties = publ
                                .topic_name()
                                .trim_start_matches(&cloned_self.rx_topic_prefix);
                            let property_tuples =
                                serde_urlencoded::from_str::<Vec<(String, String)>>(properties)
                                    .unwrap();
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

                            if handler_tx
                                .send(MessageType::C2DMessage(message))
                                .await
                                .is_err()
                            {
                                break;
                            }

                            continue;
                        }

                        #[cfg(feature = "twin-properties")]
                        if publ.topic_name().starts_with(TWIN_PATCH_TOPIC_PREFIX) {
                            // Twin update
                            if handler_tx
                                .send(MessageType::DesiredPropertyUpdate(message))
                                .await
                                .is_err()
                            {
                                break;
                            }

                            continue;
                        }

                        #[cfg(feature = "direct-methods")]
                        if publ.topic_name().starts_with(METHOD_POST_TOPIC_PREFIX) {
                            // Direct method invocation
                            // Sent to topic in format $iothub/methods/POST/{method name}/?$rid={request id}

                            // Strip the prefix from the topic left with {method name}/$rid={request id}
                            let details = &publ.topic_name()[METHOD_POST_TOPIC_PREFIX.len()..];

                            let method_components: Vec<_> = details.split('/').collect();

                            let request_id =
                                method_components[1][REQUEST_ID_PARAM.len()..].to_string();

                            if handler_tx
                                .send(MessageType::DirectMethod(DirectMethodInvocation {
                                    method_name: method_components[0].to_string(),
                                    message,
                                    request_id: request_id,
                                }))
                                .await
                                .is_err()
                            {
                                break;
                            }

                            continue;
                        }
                    }
                    _ => {}
                }
            }

            // If mpsc::Sender::send fails, it'll break the loop
            // From the docs, the send operation can only fail if the receiving ennd of a channel is disconnected
            // If the receiver has been dropped, stop receiving loop and send MQTT unsubscribe
            cloned_self.unsubscribe().await;
        });

        self.subscribe().await;

        // Send empty message so hub will respond with device twin data
        #[cfg(feature = "twin-properties")]
        self.request_twin_properties("0").await.unwrap();

        // let (abort_handle, abort_registration) = AbortHandle::new_pair();
        // let _ = Abortable::new(handle, abort_registration);
        // self.rx_loop_handle = Some(abort_handle);

        handler_rx
    }
}

#[cfg(any(
    feature = "direct-methods",
    feature = "c2d-messages",
    feature = "twin-properties",
))]
impl MqttTransport {
    async fn subscribe(&mut self) {
        let topics = vec![
            #[cfg(feature = "direct-methods")]
            (
                TopicFilter::new(METHOD_POST_TOPIC_FILTER).unwrap(),
                QualityOfService::Level0,
            ),
            #[cfg(feature = "c2d-messages")]
            (
                TopicFilter::new(device_bound_messages_topic_filter(&self.device_id)).unwrap(),
                QualityOfService::Level0,
            ),
            #[cfg(feature = "twin-properties")]
            (
                TopicFilter::new(TWIN_RESPONSE_TOPIC_FILTER).unwrap(),
                QualityOfService::Level0,
            ),
            #[cfg(feature = "twin-properties")]
            (
                TopicFilter::new(TWIN_PATCH_TOPIC_FILTER).unwrap(),
                QualityOfService::Level0,
            ),
        ];

        trace!("Subscribing to {:?}", topics);

        let subscribe_packet = SubscribePacket::new(10, topics);
        let mut buf = Vec::new();
        subscribe_packet.encode(&mut buf).unwrap();
        self.write_socket
            .lock()
            .await
            .write_all(&buf[..])
            .await
            .unwrap();
    }

    async fn unsubscribe(&mut self) {
        let topics = vec![
            #[cfg(feature = "direct-methods")]
            TopicFilter::new(METHOD_POST_TOPIC_FILTER).unwrap(),
            #[cfg(feature = "c2d-messages")]
            TopicFilter::new(device_bound_messages_topic_filter(&self.device_id)).unwrap(),
            #[cfg(feature = "twin-properties")]
            TopicFilter::new(TWIN_RESPONSE_TOPIC_FILTER).unwrap(),
            #[cfg(feature = "twin-properties")]
            TopicFilter::new(TWIN_PATCH_TOPIC_FILTER).unwrap(),
        ];

        trace!("Unsubscribing to {:?}", topics);

        let unsubscribe_packet = UnsubscribePacket::new(10, topics);
        let mut buf = Vec::new();
        unsubscribe_packet.encode(&mut buf).unwrap();
        // If the connection is lost, do not unwrap.
        let _ = self.write_socket.lock().await.write_all(&buf[..]).await;
    }
}

fn build_topic_name(
    base_topic: &TopicName,
    message: &Message,
) -> Result<TopicName, TopicNameError> {
    let capacity = message.system_properties.len() + message.properties.len();
    let mut props = std::collections::HashMap::with_capacity(capacity);
    props.extend(message.system_properties.iter());
    props.extend(message.properties.iter());

    // if we reuse the base_topic string as the target for the serializer,
    // we end up with an extra ampersand before the key/value pairs
    let encoded = form_urlencoded::Serializer::new(String::new())
        .extend_pairs(props.iter())
        .finish();
    TopicName::new(format!("{}{}", base_topic.to_string(), encoded))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Message;

    #[test]
    fn content_type_is_appended_to_topic_name() {
        let message = Message::builder()
            .set_body(vec![])
            .set_content_type("application/json".to_owned())
            .build();

        let base_topic = TopicName::new("topic/").unwrap();

        let topic_with_properties = build_topic_name(&base_topic, &message).unwrap().to_string();

        assert_eq!("topic/%24.ct=application%2Fjson", topic_with_properties);
    }

    #[test]
    fn content_encoding_is_appended_to_topic_name() {
        let message = Message::builder()
            .set_body(vec![])
            .set_content_encoding("utf-8".to_owned())
            .build();

        let base_topic = TopicName::new("topic/").unwrap();

        let topic_with_properties = build_topic_name(&base_topic, &message).unwrap().to_string();

        assert_eq!("topic/%24.ce=utf-8", topic_with_properties);
    }

    #[test]
    fn message_id_is_appended_to_topic_name() {
        let message = Message::builder()
            .set_body(vec![])
            .set_message_id("id".to_owned())
            .build();

        let base_topic = TopicName::new("topic/").unwrap();

        let topic_with_properties = build_topic_name(&base_topic, &message).unwrap().to_string();

        assert_eq!("topic/%24.mid=id", topic_with_properties);
    }

    #[test]
    fn no_system_properties() {
        let message = Message::new(vec![]);
        let base_topic = TopicName::new("topic/").unwrap();
        let actual = build_topic_name(&base_topic, &message).unwrap();
        assert_eq!(base_topic, actual);
    }

    #[test]
    fn app_properties_are_appended_to_topic_name() {
        let message = Message::builder()
            .set_body(vec![])
            .add_message_property("foo".to_owned(), "bar".to_owned())
            .build();

        let base_topic = TopicName::new("topic/").unwrap();

        let topic_with_properties = build_topic_name(&base_topic, &message).unwrap().to_string();

        assert_eq!("topic/foo=bar", topic_with_properties);
    }
}
