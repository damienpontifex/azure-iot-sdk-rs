//! Azure IoT device client for writing iot device code in rust
//!
//! # Examples
//!
//! A simple client
//! ```no_run
//! use azure_iot_sdk::client::IoTHubClient;
//! use tokio::time;
//!
//! #[tokio::main]
//! async fn main() {
//!     let mut client =
//!     IoTHubClient::from_connection_string("HostName=iothubname.azure-devices.net;DeviceId=MyDeviceId;SharedAccessKey=TheAccessKey").await;
//!
//!     let mut interval = time::interval(time::Duration::from_secs(1));
//!     let mut count: u32 = 0;
//!
//!     loop {
//!         interval.tick().await;
//!
//!         let msg = Message::builder()
//!             .set_body_from(format!("Message #{}", count))
//!             .set_message_id(format!("{}-t", count))
//!             .build();
//!
//!         client.send_message(msg).await;
//!
//!         count += 1;
//!     }
//! }
//! ```

#[macro_use]
extern crate log;

/// IoT SDK package version
pub const SDK_VERSION: &str = std::env!("CARGO_PKG_VERSION");

pub mod client;
pub mod message;
