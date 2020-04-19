#[macro_use]
extern crate log;

pub const SDK_VERSION: &str = std::env!("CARGO_PKG_VERSION");

pub mod client;
pub mod message;
