//! Azure IoT device client for writing iot device code in rust
//!
//! ## Feature flags
//!
//! SDK client uses [feature
//! flags](https://doc.rust-lang.org/cargo/reference/features.html#the-features-section) to
//! configure capabilities of the client sdk. By default all features are enabled.
//!
//! Device to cloud messaging is always available.
//!
//! - `c2d-messages`: Enables cloud to device messaging
//! - `twin-properties`: Enables device twin property updates
//! - `direct-methods`: Enables listening for direct method invocations
//!
//! ### Disabling capabilities
//! If not all features are required, disable the default features and add only desired.
//!
//! ```toml
//! azure_iot_sdk = { version = "0.3.0", features = [], default-features = false }
//! ```
//!
//! # Examples
//!
//! A simple client
//! ```no_run
//! use azure_iot_sdk::client::IoTHubClient;
//! use azure_iot_sdk::message::Message;
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
//!             .set_body(format!("Message #{}", count).as_bytes().to_vec())
//!             .set_message_id(format!("{}-t", count))
//!             .build();
//!
//!         client.send_message(msg).await;
//!
//!         count += 1;
//!     }
//! }
//! ```

#![warn(missing_debug_implementations, rust_2018_idioms, missing_docs)]

#[macro_use]
extern crate log;

/// IoT SDK package version
pub const SDK_VERSION: &str = std::env!("CARGO_PKG_VERSION");

/// The IoT Hub client
pub mod client;
#[cfg(feature = "http-transport")]
pub(crate) mod http_transport;
/// Message types for communicating with the IoT Hub
pub mod message;
#[cfg(not(any(feature = "http-transport", feature = "amqp-transport")))]
pub(crate) mod mqtt_transport;
/// Transport types
pub mod transport;

/// Errors
pub mod errors;