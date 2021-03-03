use crate::token::DeviceKeyTokenSource;
use std::sync::{Arc, Mutex};

use crate::token::TokenSource;
#[cfg(feature = "direct-methods")]
use crate::DirectMethodResponse;
#[cfg(any(
    feature = "direct-methods",
    feature = "c2d-messages",
    feature = "twin-properties"
))]
use crate::MessageType;
use crate::{message::Message, D2C};
#[cfg(any(
    feature = "direct-methods",
    feature = "c2d-messages",
    feature = "twin-properties"
))]
use tokio::sync::mpsc::Receiver;

//#[cfg(not(feature = "http-transport"))]
//pub(crate) type ClientTransport = crate::transport::MqttTransport;

/// Client for communicating with IoT hub
#[derive(Debug, Clone)]
pub struct IoTHubClient {
    device_id: String,
    // transport: ClientTransport,
    transport_handle: Arc<tokio::task::JoinHandle<()>>,
    d2c_sender: tokio::sync::mpsc::Sender<D2C>,
    c2d_receiver: Arc<Mutex<tokio::sync::mpsc::Receiver<MessageType>>>,
}

impl Drop for IoTHubClient {
    fn drop(&mut self) {
        self.transport_handle.abort();
    }
}

impl IoTHubClient {
    /// # Arguments
    ///
    /// * `hub_name` -
    /// * `device_id` -
    /// * `device_key` -
    pub async fn new_with_device_key(
        hub_name: &str,
        device_id: String,
        device_key: String,
    ) -> crate::Result<IoTHubClient> {
        let token_source = DeviceKeyTokenSource::new(hub_name, &device_id, device_key).unwrap();
        Self::new(hub_name, device_id, token_source).await
    }
    /// Create a new IoT Hub device client using a shared access signature
    ///
    /// # Arguments
    ///
    /// * `hub_name` - The IoT hub resource name
    /// * `device_id` - The registered device to connect as
    /// * `token_source` - The token source to provide authentication
    ///
    /// # Example
    /// ```no_run
    /// use azure_iot_sdk::{IoTHubClient, DeviceKeyTokenSource};
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let iothub_hostname = "iothubname.azure-devices.net";
    ///     let device_id = "MyDeviceId";
    ///     let token_source = DeviceKeyTokenSource::new(
    ///         iothub_hostname,
    ///         device_id,
    ///         "TheAccessKey",
    ///     ).unwrap();
    ///
    ///     let mut client =
    ///         IoTHubClient::new(iothub_hostname, device_id.into(), token_source).await;
    /// }
    /// ```
    pub async fn new<TS>(
        hub_name: &str,
        device_id: String,
        token_source: TS,
    ) -> crate::Result<IoTHubClient>
    where
        TS: TokenSource + Send + Clone + 'static + Sync,
    {
        let (tx, rx) = tokio::sync::mpsc::channel(1024);
        let (tx1, rx1) = tokio::sync::mpsc::channel(1024);
        let handle = crate::transport::mqtt::init(
            hub_name.to_string(),
            device_id.clone(),
            token_source,
            rx,
            tx1,
        )
        .await
        .unwrap();

        Ok(Self {
            device_id,
            transport_handle: Arc::new(handle),
            d2c_sender: tx,
            c2d_receiver: Arc::new(Mutex::new(rx1)),
        })
    }

    /// Send a device to cloud message for this device to the IoT Hub
    ///
    /// # Example
    /// ```no_run
    /// use tokio::time;
    /// use azure_iot_sdk::{IoTHubClient, DeviceKeyTokenSource, Message};
    ///
    /// #[tokio::main]
    /// async fn main() -> azure_iot_sdk::Result<()> {
    ///     let iothub_hostname = "iothubname.azure-devices.net";
    ///     let device_id = "MyDeviceId";
    ///     let token_source = DeviceKeyTokenSource::new(
    ///         iothub_hostname,
    ///         device_id,
    ///         "TheAccessKey",
    ///     ).unwrap();
    ///
    ///     let mut client =
    ///         IoTHubClient::new(iothub_hostname, device_id.into(), token_source).await?;
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
    ///         client.send_message(msg).await?;
    ///
    ///         count += 1;
    ///     }
    /// }
    /// ```
    pub async fn send_message(&mut self, message: Message) -> crate::Result<()> {
        // self.transport.send_message(message).await
        self.d2c_sender.send(D2C::Telemetry(message)).await.unwrap();
        Ok(())
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
    ///    client.send_property_update(&format!("{}", update_counter), &body).await.unwrap();
    ///    update_counter += 1;
    /// ```
    #[cfg(feature = "twin-properties")]
    pub async fn send_property_update(
        &mut self,
        request_id: &str,
        body: &str,
    ) -> crate::Result<()> {
        // self.transport.send_property_update(request_id, body).await
        todo!()
    }

    ///
    #[cfg(any(
        feature = "direct-methods",
        feature = "c2d-messages",
        feature = "twin-properties"
    ))]
    pub async fn get_receiver(&mut self) -> Arc<Mutex<Receiver<MessageType>>> {
        self.c2d_receiver.clone()
    }

    ///
    #[cfg(feature = "direct-methods")]
    pub async fn respond_to_direct_method(
        &mut self,
        response: DirectMethodResponse,
    ) -> crate::Result<()> {
        // self.transport.respond_to_direct_method(response).await
        todo!()
    }
}
