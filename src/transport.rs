use crate::{client::TokenSource, message::Message};
use async_trait::async_trait;

///
#[async_trait]
pub trait Transport {
    ///
    async fn new<TS>(hub_name: &str, device_id: &str, token_source: TS) -> Self
    where
        TS: TokenSource + Sync + Send;
    ///
    async fn send_message(&mut self, message: Message);
    ///
    async fn send_property_update(&mut self, request_id: &str, body: &str);
    ///
    async fn set_message_handler(&mut self, device_id: &str, handler: MessageHandler);
}

///
// #[derive(Debug)]
pub enum MessageHandler {
    ///
    Message(Box<dyn Fn(Message) + Send>),
    ///
    TwinUpdate(Box<dyn Fn(Message) + Send>),
    ///
    DirectMethod(Box<dyn Fn(String, Message) -> i32 + Send>),
}
