# Azure IoT SDK for Rust

Self developed library to interact with [Azure IoT Hub using MQTT protocol](https://docs.microsoft.com/en-us/azure/iot-hub/iot-hub-mqtt-support)

![CI](https://github.com/damienpontifex/azure-iot-sdk-rs/workflows/CI/badge.svg)
[![docs](https://docs.rs/azure_iot_sdk/badge.svg)](https://docs.rs/azure_iot_sdk)
[![Crate](https://img.shields.io/crates/v/azure_iot_sdk.svg)](https://crates.io/crates/azure_iot_sdk)
[![cratedown](https://img.shields.io/crates/d/azure_iot_sdk.svg)](https://crates.io/crates/azure_iot_sdk)
[![cratelastdown](https://img.shields.io/crates/dv/azure_iot_sdk.svg)](https://crates.io/crates/azure_iot_sdk)

## Running examples
Copy the sample config file
```bash
cp examples/config.sample.toml examples/config.toml
```

Edit values in examples/config.toml with your iot hub host, device and primary key
