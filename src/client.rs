use crate::dtmi::Dtmi;
use crate::message::Message;
#[cfg(feature = "direct-methods")]
use crate::DirectMethodResponse;
#[cfg(any(
    feature = "direct-methods",
    feature = "c2d-messages",
    feature = "twin-properties"
))]
use crate::MessageType;
use crate::{token::TokenSource, transport::Transport};
#[cfg(any(
    feature = "direct-methods",
    feature = "c2d-messages",
    feature = "twin-properties"
))]
use tokio::sync::mpsc::Receiver;

#[cfg(not(feature = "https-transport"))]
pub(crate) type ClientTransport = crate::mqtt_transport::MqttTransport;
#[cfg(feature = "https-transport")]
pub(crate) type ClientTransport = crate::http_transport::HttpsTransport;

/// Client for communicating with IoT hub
#[derive(Clone)]
pub struct IoTHubClient {
    device_id: String,
    transport: ClientTransport,
}

impl std::fmt::Debug for IoTHubClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        #[cfg(not(feature = "https-transport"))]
        let transport_debug = "MQTT";
        #[cfg(feature = "https-transport")]
        let transport_debug = "HTTP";
        f.debug_struct("IoTHubClient")
            .field("device_id", &self.device_id)
            .field("transport", &transport_debug)
            .finish()
    }
}

impl IoTHubClient {
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
        TS: TokenSource + Send + Sync + Clone + 'static,
    {
        Self::builder(hub_name, device_id, token_source)
            .build()
            .await
    }

    /// Create a new builder
    pub fn builder<TS>(
        hub_name: &str,
        device_id: String,
        token_source: TS,
    ) -> IoTHubClientBuilder<TS> {
        IoTHubClientBuilder {
            hub_name: hub_name.into(),
            device_id,
            token_source,
            root_ca: None,
            pnp_model_id: None,
        }
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
        self.transport.send_message(message).await
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
        self.transport.send_property_update(request_id, body).await
    }

    ///
    #[cfg(any(
        feature = "direct-methods",
        feature = "c2d-messages",
        feature = "twin-properties"
    ))]
    pub async fn get_receiver(&mut self) -> Receiver<MessageType> {
        self.transport.get_receiver().await
    }

    ///
    #[cfg(feature = "direct-methods")]
    pub async fn respond_to_direct_method(
        &mut self,
        response: DirectMethodResponse,
    ) -> crate::Result<()> {
        self.transport.respond_to_direct_method(response).await
    }

    ///
    pub async fn ping(&mut self) -> crate::Result<()> {
        self.transport.ping().await
    }
}

/// A builder for an IoTHubClient
#[allow(missing_debug_implementations)]
pub struct IoTHubClientBuilder<TS> {
    hub_name: String,
    device_id: String,
    token_source: TS,
    root_ca: Option<native_tls::Certificate>,
    pnp_model_id: Option<Dtmi>,
}

impl<TS: TokenSource + Send + Sync + Clone + 'static> IoTHubClientBuilder<TS> {
    /// Build the IoTHubClient.
    ///
    /// Consumes the builder.
    pub async fn build(self) -> crate::Result<IoTHubClient> {
        let IoTHubClientBuilder {
            hub_name,
            device_id,
            token_source,
            root_ca,
            pnp_model_id,
        } = self;

        #[cfg(not(feature = "https-transport"))]
        let transport = ClientTransport::new(
            &hub_name,
            device_id.clone(),
            token_source.clone(),
            root_ca,
            pnp_model_id,
        )
        .await?;
        #[cfg(feature = "https-transport")]
        let transport =
            ClientTransport::new(&hub_name, device_id.clone(), token_source.clone()).await?;

        Ok(IoTHubClient {
            device_id,
            transport,
        })
    }

    /// Add a certificate to the set of roots that the client will trust
    ///
    /// The provided certificate must be PEM encoded
    pub fn root_ca(mut self, root_ca: impl AsRef<[u8]>) -> crate::Result<Self> {
        let root_ca = native_tls::Certificate::from_pem(root_ca.as_ref())?;
        self.root_ca = Some(root_ca);
        Ok(self)
    }

    /// Provide a PNP model ID
    ///
    /// Is only use with MQTT transport
    pub fn pnp_model_id(mut self, pnp_model_id: impl AsRef<str>) -> crate::Result<Self> {
        let dtmi = pnp_model_id.as_ref().parse()?;
        self.pnp_model_id = Some(dtmi);
        Ok(self)
    }
}
