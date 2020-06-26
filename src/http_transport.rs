// use crate::client::TokenSource;
// use crate::message::Message;
// use crate::transport::{MessageHandler, Transport};
// use async_trait::async_trait;
// use chrono::{Duration, Utc};
// use hyper::{client::HttpConnector, header, Body, Client, Method, Request};
// use hyper_tls::HttpsConnector;

// #[derive(Debug, Clone)]
// pub(crate) struct HttpTransport<TS> {
//     hub_name: String,
//     device_id: String,
//     token_source: TS,
//     client: Client<HttpsConnector<HttpConnector>>,
// }

// #[async_trait]
// impl<TS> Transport for HttpTransport<TS>
// where
//     TS: TokenSource + Sync + Send,
// {
//     async fn new(hub_name: &str, device_id: &str, token_source: TS) -> Self
//     where
//         TS: TokenSource + Sync + Send,
//     {
//         let https = HttpsConnector::new();
//         let client = Client::builder().build::<_, hyper::Body>(https);
//         HttpTransport {
//             hub_name: hub_name.to_string(),
//             device_id: device_id.to_string(),
//             token_source,
//             client,
//         }
//     }

//     async fn send_message(&mut self, message: Message) {
//         let token_lifetime = Utc::now() + Duration::days(1);
//         let req = Request::builder()
//             .method(Method::POST)
//             .uri(format!(
//                 "https://{}/devices/{}/messages/events?api-version=2019-03-30",
//                 self.hub_name, self.device_id
//             ))
//             .header(header::CONTENT_TYPE, "application/json")
//             .header(
//                 header::AUTHORIZATION,
//                 self.token_source.get(&token_lifetime),
//             )
//             .body(Body::from(message.body))
//             .unwrap();

//         let res = self.client.request(req).await.unwrap();

//         debug!("Response: {}", res.status());
//     }

//     async fn send_property_update(&mut self, _request_id: &str, _body: &str) {
//         unimplemented!()
//     }

//     async fn set_message_handler(&mut self, _device_id: &str, _handler: MessageHandler) {
//         unimplemented!()
//     }
// }
