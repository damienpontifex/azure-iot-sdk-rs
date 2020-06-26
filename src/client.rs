use crate::message::Message;
use crate::{
    token::TokenSource,
    transport::{MessageHandler, Transport},
};

/// Client for communicating with IoT hub
#[derive(Debug, Clone)]
pub struct IoTHubClient<'a, TR>
where
    TR: Transport,
{
    device_id: &'a str,
    transport: TR,
}

impl<'a, TR> IoTHubClient<'a, TR>
where
    TR: Transport,
{
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
    /// use azure_iot_sdk::{IoTHubClient, DeviceKeyTokenSource, MqttTransport};
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
    ///         IoTHubClient::<MqttTransport>::new(iothub_hostname, device_id, token_source).await;
    /// }
    /// ```
    pub async fn new<TS>(
        hub_name: &str,
        device_id: &'a str,
        token_source: TS,
    ) -> IoTHubClient<'a, TR>
    where
        TS: TokenSource + Sync + Send,
    {
        let transport = TR::new(hub_name, device_id, token_source).await;
        //         #[cfg(not(any(feature = "http-transport", feature = "amqp-transport")))]
        //         let transport = MqttTransport::new(hub_name, device_id, token_source).await;
        //
        //         #[cfg(feature = "http-transport")]
        //         let transport = HttpTransport::new(hub_name, device_id, token_source).await;

        Self {
            device_id,
            transport,
        }
    }

    /// Send a device to cloud message for this device to the IoT Hub
    ///
    /// # Example
    /// ```no_run
    /// use tokio::time;
    /// use azure_iot_sdk::{IoTHubClient, DeviceKeyTokenSource, MqttTransport, Message};
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
    ///         IoTHubClient::<MqttTransport>::new(iothub_hostname, device_id, token_source).await;
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
    /// use azure_iot_sdk::{IoTHubClient, DeviceKeyTokenSource, MqttTransport};
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
    ///         IoTHubClient::<MqttTransport>::new(iothub_hostname, device_id, token_source).await;
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
    /// use azure_iot_sdk::{IoTHubClient, DeviceKeyTokenSource, MqttTransport};
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
    ///         IoTHubClient::<MqttTransport>::new(iothub_hostname, device_id, token_source).await;
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
    /// use azure_iot_sdk::{IoTHubClient, DeviceKeyTokenSource, MqttTransport};
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
    ///         IoTHubClient::<MqttTransport>::new(iothub_hostname, device_id, token_source).await;
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
