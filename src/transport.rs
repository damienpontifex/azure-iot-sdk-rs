use crate::{message::Message, token::TokenSource, DirectMethodResponse, MessageType};
use async_trait::async_trait;
use tokio::sync::mpsc::Receiver;

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
    async fn request_twin_properties(&mut self, request_id: &str);

    ///
    async fn respond_to_direct_method(&mut self, response: DirectMethodResponse);

    ///
    async fn ping(&mut self);

    async fn get_receiver(&mut self) -> Receiver<MessageType>;
}
