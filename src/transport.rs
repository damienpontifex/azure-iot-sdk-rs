use crate::message::Message;
use async_trait::async_trait;
#[async_trait]
pub(crate) trait Transport {
    async fn new(hub_name: &str, device_id: &str, sas: String) -> Self;
    async fn send_message(&mut self, message: Message);
    async fn send_property_update(&mut self, request_id: &str, body: &str);
    async fn set_message_handler(&mut self, device_id: &str, handler: MessageHandler);
}

pub(crate) enum MessageHandler {
    Message(Box<dyn Fn(Message) + Send>),
    TwinUpdate(Box<dyn Fn(Message) + Send>),
    DirectMethod(Box<dyn Fn(String, Message) -> i32 + Send>),
}
