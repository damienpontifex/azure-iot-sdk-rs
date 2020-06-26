#[macro_use]
extern crate log;

use azure_iot_sdk::{
    client::IoTHubClient, message::Message, mqtt_transport::MqttTransport,
    token::DeviceKeyTokenSource,
};

use chrono::{DateTime, Utc};
use tokio::time;

use serde::{Deserialize, Serialize};

use rand_distr::{Distribution, Normal};

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
    timestamp: DateTime<Utc>,
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
            timestamp: Utc::now(),
            temperature: self.distribution.sample(&mut rand::thread_rng()),
        }
    }
}

#[derive(Serialize, Debug)]
struct HumidityReading {
    timestamp: DateTime<Utc>,
    humidity: f32,
}

struct HumiditySensor {
    distribution: Normal<f32>,
}

impl HumiditySensor {
    fn default() -> Self {
        Self {
            distribution: Normal::new(50.0, 7.0).unwrap(),
        }
    }

    fn get_reading(&self) -> HumidityReading {
        HumidityReading {
            timestamp: Utc::now(),
            humidity: self.distribution.sample(&mut rand::thread_rng()),
        }
    }
}

#[tokio::main]
async fn main() {
    env_logger::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let config = DeviceConfig::from_env().unwrap();

    let token_source = DeviceKeyTokenSource::new(
        &config.hostname,
        &config.device_id,
        &config.shared_access_key,
    );

    let mut client =
        IoTHubClient::<MqttTransport>::new(&config.hostname, &config.device_id, token_source).await;

    info!("Initialized client");

    client
        .on_message(|msg| {
            println!("Received message {:?}", msg);
        })
        .await;

    client
        .on_direct_method(|method_name, msg| {
            println!(
                "Received direct method {} {}",
                method_name,
                std::str::from_utf8(&msg.body).unwrap()
            );
            0
        })
        .await;

    // let mut rx = client.get_receiver().unwrap();
    // let mut tx = client.sender();
    // let receiver = async move {
    //     while let Some(msg) = rx.recv().await {
    //         match msg {
    //             MessageType::C2DMessage(message) => info!("Received message {:?}", message),
    //             MessageType::DirectMethod(direct_method) => {
    //                 info!("Received direct method {:?}", direct_method);
    //                 tx.send(SendType::RespondToDirectMethod(DirectMethodResponse::new(
    //                     direct_method.request_id,
    //                     0,
    //                     Some("{'responseKey': 1}".into()),
    //                 )))
    //                 .await
    //                 .unwrap();
    //             }
    //             MessageType::DesiredPropertyUpdate(property_update) => {
    //                 let json: serde_json::Value = property_update.into();
    //                 info!("Received property update {:?}", json);
    //             }
    //         }
    //     }
    // };

    let mut interval = time::interval(time::Duration::from_secs(2));
    let mut count = 0u32;
    let sensor = TemperatureSensor::default();

    let mut temp_client = client.clone();
    let temp_sender = async {
        loop {
            interval.tick().await;

            let temp = sensor.get_reading();

            let msg = Message::builder()
                .set_body(serde_json::to_vec(&temp).unwrap())
                .set_message_id(format!("{}-t", count))
                .build();

            temp_client.send_message(msg).await;

            count += 1;
        }
    };

    let mut interval = time::interval(time::Duration::from_secs(5));
    let mut count = 0u32;
    let sensor = HumiditySensor::default();

    let humidity_sender = async move {
        loop {
            interval.tick().await;

            let humidity = sensor.get_reading();

            let msg = Message::builder()
                .set_body(serde_json::to_vec(&humidity).unwrap())
                .set_message_id(format!("{}-h", count))
                .build();

            client.send_message(msg).await;

            count += 1;
        }
    };

    futures::join!(temp_sender, humidity_sender);
}
