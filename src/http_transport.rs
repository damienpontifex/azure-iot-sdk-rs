use crate::client::TokenSource;
use crate::message::Message;
use crate::transport::{MessageHandler, Transport};
use async_trait::async_trait;
use hyper::{client::HttpConnector, header, Body, Client, Method, Request};
use hyper_tls::HttpsConnector;

#[derive(Debug, Clone)]
pub(crate) struct HttpTransport {
    hub_name: String,
    device_id: String,
    sas: String,
    client: Client<HttpsConnector<HttpConnector>>,
}

#[async_trait]
impl Transport for HttpTransport {
    async fn new<TS>(hub_name: &str, device_id: &str, token_source: &TS) -> Self
    where
        TS: TokenSource + Sync + Send,
    {
        let https = HttpsConnector::new();
        let client = Client::builder().build::<_, hyper::Body>(https);
        HttpTransport {
            hub_name: hub_name.to_string(),
            device_id: device_id.to_string(),
            sas,
            client,
        }
    }

    async fn send_message(&mut self, message: Message) {
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

        let res = self.client.request(req).await.unwrap();

        debug!("Response: {}", res.status());
    }

    async fn send_property_update(&mut self, _request_id: &str, _body: &str) {
        unimplemented!()
    }

    async fn set_message_handler(&mut self, _device_id: &str, _handler: MessageHandler) {
        unimplemented!()
    }
}
