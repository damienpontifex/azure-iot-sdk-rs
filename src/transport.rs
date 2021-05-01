use crate::message::Message;

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
    async fn send_message(&mut self, message: Message) -> crate::Result<()>;
    ///
    #[cfg(feature = "twin-properties")]
    async fn send_property_update(&mut self, request_id: &str, body: &str) -> crate::Result<()>;

    ///
    #[cfg(feature = "twin-properties")]
    async fn request_twin_properties(&mut self, request_id: &str) -> crate::Result<()>;

    ///
    #[cfg(feature = "direct-methods")]
    async fn respond_to_direct_method(
        &mut self,
        response: DirectMethodResponse,
    ) -> crate::Result<()>;

    ///
    async fn ping(&mut self) -> crate::Result<()>;

    ///
    #[cfg(any(
        feature = "direct-methods",
        feature = "c2d-messages",
        feature = "twin-properties"
    ))]
    async fn get_receiver(&mut self) -> Receiver<MessageType>;
}
