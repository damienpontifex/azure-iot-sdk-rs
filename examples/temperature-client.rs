#[macro_use]
extern crate log;

use azure_iot_sdk::{
    client::IoTHubClient,
    message::{DirectMethodResponse, Message, MessageType, SendType},
};

use tokio::time;

use serde::{Deserialize, Serialize};

use rand_distr::{Distribution, Normal};

use config;

#[derive(Debug, Deserialize)]
struct DeviceConfig {
    hostname: String,
    device_id: String,
    shared_access_key: String,
}

impl DeviceConfig {
    fn from_env() -> Result<Self, config::ConfigError> {
        let mut cfg = config::Config::default();
        cfg.merge(config::File::with_name("examples/config"))?;
        cfg.try_into()
    }
}

#[derive(Serialize, Debug)]
struct TemperatureReading {
    temperature: f32,
}

struct TemperatureSensor {
    distribution: Normal<f32>,
}

impl TemperatureSensor {
    fn default() -> Self {
        TemperatureSensor {
            distribution: Normal::new(25.0, 7.0).unwrap(),
        }
    }

    fn get_reading(&self) -> TemperatureReading {
        TemperatureReading {
            temperature: self.distribution.sample(&mut rand::thread_rng()),
        }
    }
}

#[tokio::main]
async fn main() {
    env_logger::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let config = DeviceConfig::from_env().unwrap();

    let mut client =
        IoTHubClient::with_device_key(config.hostname, config.device_id, config.shared_access_key)
            .await;

    info!("Initialized client");

    client.on_message(|msg| {
        println!("Received message");
    });

    client.on_direct_method(|method_name, msg| {
        println!("Received direct method invocation for {}", method_name);
        0
    });

    let mut rx = client.get_receiver().unwrap();
    let mut tx = client.sender();
    let receiver = async move {
        while let Some(msg) = rx.recv().await {
            match msg {
                MessageType::C2DMessage(message) => info!("Received message {:?}", message),
                MessageType::DirectMethod(direct_method) => {
                    info!("Received direct method {:?}", direct_method);
                    tx.send(SendType::RespondToDirectMethod(DirectMethodResponse::new(
                        direct_method.request_id,
                        0,
                        Some("{'responseKey': 1}".into()),
                    )))
                    .await
                    .unwrap();
                }
                MessageType::DesiredPropertyUpdate(property_update) => {
                    let json: serde_json::Value = property_update.into();
                    info!("Received property update {:?}", json);
                }
            }
        }
    };

    let mut interval = time::interval(time::Duration::from_secs(1));
    let mut count = 0u32;
    let sensor = TemperatureSensor::default();

    let sender = async move {
        loop {
            interval.tick().await;

            let temp = sensor.get_reading();

            let msg = Message::builder()
                .set_body_from(temp)
                .set_message_id(format!("{}-t", count))
                .build();

            client.send_message(msg).await;

            count += 1;
        }
    };

    futures::join!(sender, receiver);
}
