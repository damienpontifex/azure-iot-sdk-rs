use std::collections::HashMap;

/// Type of message received in cloud to device communication
#[cfg(any(
    feature = "direct-methods",
    feature = "c2d-messages",
    feature = "twin-properties"
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

// System properties that are user settable
// https://docs.microsoft.com/bs-cyrl-ba/azure/iot-hub/iot-hub-devguide-messages-construct#system-properties-of-d2c-iot-hub-messages
const MESSAGE_ID: &str = "message-id";

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
    pub fn set_message_id(mut self, message_id: String) -> Self {
        self.system_properties
            .insert(MESSAGE_ID.to_owned(), message_id);
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
