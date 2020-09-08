use crate::{message::Message, token::TokenSource};

#[cfg(feature = "direct-methods")]
use crate::message::DirectMethodResponse;
#[cfg(any(
    feature = "direct-methods",
    feature = "c2d-messages",
    feature = "twin-properties"
))]
use crate::message::MessageType;
use async_trait::async_trait;
#[cfg(any(
    feature = "direct-methods",
    feature = "c2d-messages",
    feature = "twin-properties"
))]
use tokio::sync::mpsc::Receiver;

///
#[async_trait]
pub trait Transport {
    ///
    async fn new<TS>(hub_name: &str, device_id: &str, token_source: TS) -> Self
    where
        TS: TokenSource + Sync + Send;
    ///
    async fn send_message(&mut self, message: Message) -> std::io::Result<()>;
    ///
    #[cfg(feature = "twin-properties")]
    async fn send_property_update(&mut self, request_id: &str, body: &str);

    ///
    #[cfg(feature = "twin-properties")]
    async fn request_twin_properties(&mut self, request_id: &str);

    ///
    #[cfg(feature = "direct-methods")]
    async fn respond_to_direct_method(&mut self, response: DirectMethodResponse);

    ///
    async fn ping(&mut self);

    ///
    #[cfg(any(
        feature = "direct-methods",
        feature = "c2d-messages",
        feature = "twin-properties"
    ))]
    async fn get_receiver(&mut self) -> Receiver<MessageType>;
}
