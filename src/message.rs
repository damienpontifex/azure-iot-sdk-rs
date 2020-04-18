use serde::Serialize;
use std::collections::HashMap;

/// Type of message received in cloud to device communication
#[derive(Debug)]
pub enum MessageType {
    C2DMessage(Message),
    DesiredPropertyUpdate(Message),
    DirectMethod(DirectMethodInvokation),
}

#[derive(Debug)]
pub struct DirectMethodResponse {
    pub status: i32,
    pub request_id: String,
    pub body: String,
}

impl DirectMethodResponse {
    pub fn new(request_id: String, status: i32, body: Option<String>) -> Self {
        DirectMethodResponse {
            status,
            request_id,
            body: body.unwrap_or_default(),
        }
    }
}

/// The type of message to send for device to cloud communication
#[derive(Debug)]
pub enum SendType {
    Message(Message),
    Ping,
    RespondToDirectMethod(DirectMethodResponse),
}

// System properties that are user settable
// https://docs.microsoft.com/bs-cyrl-ba/azure/iot-hub/iot-hub-devguide-messages-construct#system-properties-of-d2c-iot-hub-messages
const MESSAGE_ID: &str = "message-id";

/// Message used in body of communication
#[derive(Default, Debug)]
pub struct Message {
    pub body: String,
    pub properties: HashMap<String, String>,
    pub system_properties: HashMap<String, String>,
}

impl Message {
    pub fn new(body: String) -> Self {
        Message {
            body,
            ..Default::default()
        }
    }

    pub fn from<T>(object: T) -> Self
    where
        T: Serialize,
    {
        Self::new(serde_json::to_string(&object).unwrap())
    }

    pub fn builder() -> MessageBuilder {
        MessageBuilder::default()
    }
}

#[derive(Default)]
pub struct MessageBuilder {
    message: Option<String>,
    properties: HashMap<String, String>,
    system_properties: HashMap<String, String>,
}
impl MessageBuilder {
    pub fn set_body(mut self, body: String) -> Self {
        self.message = Some(body);
        self
    }

    pub fn set_body_from<T>(mut self, object: T) -> Self
    where
        T: Serialize,
    {
        self.message = Some(serde_json::to_string(&object).unwrap());
        self
    }

    pub fn set_message_id(mut self, message_id: String) -> Self {
        self.system_properties
            .insert(MESSAGE_ID.to_owned(), message_id);
        self
    }

    pub fn add_message_property(mut self, key: String, value: String) -> Self {
        self.properties.insert(key, value);
        self
    }

    pub fn build(self) -> Message {
        Message {
            body: self.message.unwrap(),
            properties: self.properties,
            system_properties: self.system_properties,
        }
    }
}

/// Details about a cloud to device direct method invocation call
#[derive(Debug)]
pub struct DirectMethodInvokation {
    pub method_name: String,
    pub message: Message,
    pub request_id: String,
}
