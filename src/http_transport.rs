use crate::message::Message;
use crate::transport::{MessageHandler, Transport};
use async_trait::async_trait;
use hyper::{header, Body, Client, Method, Request};
use hyper_tls::HttpsConnector;

#[derive(Debug, Clone)]
pub(crate) struct HttpTransport {
    hub_name: String,
    device_id: String,
    sas: String,
}

#[async_trait]
impl Transport for HttpTransport {
    async fn new(hub_name: String, device_id: String, sas: String) -> Self {
        HttpTransport {
            hub_name,
            device_id,
            sas,
        }
    }

    async fn send_message(&mut self, message: Message) {
        let https = HttpsConnector::new();
        let client = Client::builder().build::<_, hyper::Body>(https);

        let req = Request::builder()
            .method(Method::POST)
            .uri(format!(
                "https://{}/devices/{}/messages/events?api-version=2019-03-30",
                self.hub_name, self.device_id
            ))
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::AUTHORIZATION, self.sas.clone())
            .body(Body::from(message.body))
            .unwrap();

        let res = client.request(req).await.unwrap();

        debug!("Response: {}", res.status());
    }

    async fn send_property_update(&mut self, request_id: &str, body: &str) {
        unimplemented!()
    }

    async fn set_message_handler(&mut self, device_id: &str, handler: MessageHandler) {
        unimplemented!()
    }
}
