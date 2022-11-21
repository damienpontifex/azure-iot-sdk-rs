use azure_iot_sdk::{
    DeviceKeyTokenSource, DirectMethodResponse, IoTHubClient, Message, MessageType,
};
use chrono::{DateTime, Utc};
use log::{error, info};
use rand_distr::{Distribution, Normal};
use serde::Serialize;
use tokio::time;

#[derive(Serialize, Debug)]
struct TemperatureReading {
    timestamp: DateTime<Utc>,
    temperature: f32,
}

struct TemperatureSensor {
    distribution: Normal<f32>,
}

impl Default for TemperatureSensor {
    fn default() -> Self {
        TemperatureSensor {
            distribution: Normal::new(25.0, 7.0).unwrap(),
        }
    }
}

impl TemperatureSensor {
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

impl Default for HumiditySensor {
    fn default() -> Self {
        Self {
            distribution: Normal::new(50.0, 7.0).unwrap(),
        }
    }
}

impl HumiditySensor {
    fn get_reading(&self) -> HumidityReading {
        HumidityReading {
            timestamp: Utc::now(),
            humidity: self.distribution.sample(&mut rand::thread_rng()),
        }
    }
}

#[tokio::main]
async fn main() -> azure_iot_sdk::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let hostname = std::env::var("IOTHUB_HOSTNAME")
        .expect("Set IoT Hub hostname in the IOTHUB_HOSTNAME environment variable");
    let device_id = std::env::var("DEVICE_ID")
        .expect("Set the device id in the DEVICE_ID environment variable");
    let shared_access_key = std::env::var("SHARED_ACCESS_KEY")
        .expect("Set the device shared access key in the SHARED_ACCESS_KEY environment variable");

    let token_source =
        DeviceKeyTokenSource::new(&hostname, &device_id, &shared_access_key).unwrap();

    let mut client = IoTHubClient::new(&hostname, device_id, token_source.into()).await?;

    info!("Initialized client");

    let mut recv_client = client.clone();
    let mut receiver = client.get_receiver().await;
    let receive_loop = async {
        loop {
            while let Some(msg) = receiver.recv().await {
                match msg {
                    MessageType::C2DMessage(msg) => info!("Received message {:?}", msg),
                    MessageType::DirectMethod(msg) => {
                        info!("Received direct method {:?}", msg);
                        recv_client
                            .respond_to_direct_method(DirectMethodResponse::new(
                                msg.request_id,
                                0,
                                Some(std::str::from_utf8(&msg.message.body).unwrap().to_string()),
                            ))
                            .await
                            .unwrap();
                    }
                    MessageType::DesiredPropertyUpdate(msg) => {
                        info!("Desired properties updated {:?}", msg)
                    }
                    MessageType::TwinResponse(msg) => {
                        info!("Twin response: {:?}", msg)
                    }
                    MessageType::ErrorReceive(err) => error!("Error during receive {:?}", err),
                }
            }
        }
    };

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

            temp_client.send_message(msg).await?;

            count += 1;
        }
        /*
         * We need the explicit return so the compiler can figure out the return type of the async block.
         * https://rust-lang.github.io/async-book/07_workarounds/03_err_in_async_blocks.html
         */
        #[allow(unreachable_code)]
        Ok::<(), Box<dyn std::error::Error>>(())
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

            client.send_message(msg).await?;

            count += 1;
        }

        #[allow(unreachable_code)]
        Ok::<(), Box<dyn std::error::Error>>(())
    };

    let (_, temp_sender_result, humidity_sender_result) =
        tokio::join!(receive_loop, temp_sender, humidity_sender);
    //in real life you'd do something useful, like restart the process
    temp_sender_result.unwrap();
    humidity_sender_result.unwrap();

    Ok(())
}
