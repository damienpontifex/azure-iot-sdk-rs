#[cfg(feature = "error-handling-messages")]
use mqtt::packet::VariablePacketError;
use std::collections::HashMap;

/// Type of message received in cloud to device communication
#[cfg(any(
    feature = "direct-methods",
    feature = "c2d-messages",
    feature = "twin-properties",
    feature = "error-handling-messages",
    feature = "ping-response",
))]
#[derive(Debug)]
pub enum MessageType {
    /// Cloud to device message
    #[cfg(feature = "c2d-messages")]
    C2DMessage(Message),
    /// Cloud updating desired properties
    #[cfg(feature = "twin-properties")]
    DesiredPropertyUpdate(Message),
    /// Cloud sending a direct method invocation
    #[cfg(feature = "direct-methods")]
    DirectMethod(DirectMethodInvocation),
    /// Error occurred in the message loop
    #[cfg(feature = "error-handling-messages")]
    ErrorReceive(VariablePacketError),
    /// Cloud responded to a ping
    #[cfg(feature = "ping-response")]
    PingResponse,
}

/// Instance to respond to a direct method invocation
#[cfg(feature = "direct-methods")]
#[derive(Debug)]
pub struct DirectMethodResponse {
    pub(crate) status: i32,
    pub(crate) request_id: String,
    pub(crate) body: String,
}

#[cfg(feature = "direct-methods")]
impl DirectMethodResponse {
    /// Make a new direct method response
    pub fn new(request_id: String, status: i32, body: Option<String>) -> Self {
        Self {
            status,
            request_id,
            body: body.unwrap_or_default(),
        }
    }
}

/// Message used in body of communication
#[derive(Default, Debug)]
pub struct Message {
    /// String contents of body of the message
    pub body: Vec<u8>,
    pub(crate) properties: HashMap<String, String>,
    pub(crate) system_properties: HashMap<String, String>,
}

impl Message {
    /// Create with contents of body as message bytes
    pub fn new(body: Vec<u8>) -> Self {
        Self {
            body,
            ..Default::default()
        }
    }

    /// Get a builder instance for building up a message
    pub fn builder() -> MessageBuilder {
        MessageBuilder::default()
    }
}

/// Builder for constructing Message instances
#[derive(Debug, Default)]
pub struct MessageBuilder {
    message: Option<Vec<u8>>,
    properties: HashMap<String, String>,
    system_properties: HashMap<String, String>,
}
impl MessageBuilder {
    /// Set the message body
    pub fn set_body(mut self, body: Vec<u8>) -> Self {
        self.message = Some(body);
        self
    }

    /// Set the identifier for this message
    pub fn set_message_id(self, message_id: String) -> Self {
        self.set_system_property("$.mid", message_id)
    }

    /// Set the content-type for this message, such as `text/plain`.
    /// To allow routing query on the message body, this value should be set to `application/json`
    pub fn set_content_type(self, content_type: String) -> Self {
        self.set_system_property("$.ct", content_type)
    }

    /// Set the content-encoding for this message.
    /// If the content-type is set to `application/json`, allowed values are `UTF-8`, `UTF-16`, `UTF-32`.
    pub fn set_content_encoding(self, content_encoding: String) -> Self {
        self.set_system_property("$.ce", content_encoding)
    }

    /// System properties that are user settable
    /// https://docs.microsoft.com/bs-cyrl-ba/azure/iot-hub/iot-hub-devguide-messages-construct#system-properties-of-d2c-iot-hub-messages
    /// The full list of valid "wire ids" is availabe here:
    /// https://github.com/Azure/azure-iot-sdk-csharp/blob/67f8c75576edfcbc20e23a01afc88be47552e58c/iothub/device/src/Transport/Mqtt/MqttIotHubAdapter.cs#L1068-L1086
    /// If you need to add support for a new property,
    /// you should create a new public function that sets the appropriate wire id.
    fn set_system_property(mut self, property_name: &str, value: String) -> Self {
        self.system_properties
            .insert(property_name.to_owned(), value);
        self
    }

    /// Add a message property
    pub fn add_message_property(mut self, key: String, value: String) -> Self {
        self.properties.insert(key, value);
        self
    }

    /// Build into a message instance
    pub fn build(self) -> Message {
        Message {
            body: self.message.unwrap(),
            properties: self.properties,
            system_properties: self.system_properties,
        }
    }
}

/// Details about a cloud to device direct method invocation call
#[cfg(feature = "direct-methods")]
#[derive(Debug)]
pub struct DirectMethodInvocation {
    ///
    pub method_name: String,
    ///
    pub message: Message,
    ///
    pub request_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_setting_content_type() {
        let builder = Message::builder();
        let msg = builder
            .set_content_type("application/json".to_owned())
            .set_body(vec![])
            .build();

        assert_eq!(msg.system_properties["$.ct"], "application/json");
    }

    #[test]
    fn test_setting_content_encoding() {
        let builder = Message::builder();
        let msg = builder
            .set_content_type("application/json".to_owned())
            .set_content_encoding("UTF-8".to_owned())
            .set_body(vec![])
            .build();

        assert_eq!(msg.system_properties["$.ce"], "UTF-8");
    }
}
