use std::marker::PhantomData;

use thiserror::Error;

use crate::{
    parse_connection_string, DeviceKeyTokenSource, IoTHubClient, TokenError, TokenProvider,
};

impl IoTHubClient {
    /// Get a builder for the IoT hub client
    pub fn builder() -> IoTHubClientBuilder<IoTHubClientBuilderUninitializedHubDetails> {
        IoTHubClientBuilder::default()
    }
}

/// Error related to building the client
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum IoTHubClientBuilderError {
    /// Uninitialized field
    #[error("{0} must be initialized")]
    UninitializedField(&'static str),
    /// Custom validation error
    #[error("{0} failed to validate")]
    ValidationError(&'static str),
    /// Error initializing the hub client
    #[error("An error occurred intializing the IoT Hub Client {0}")]
    ClientError(String),
}

/// The client builder has no initialized fields and requires the hub name and device id
#[derive(Debug)]
pub struct IoTHubClientBuilderUninitializedHubDetails;
/// The client builder has hub and device details and can have the token source initialized
#[derive(Debug)]
pub struct IoTHubClientBuilderInitializedHubDetails;
/// The client builder has all fields required to be built
#[derive(Debug)]
pub struct IoTHubClientBuilderInitializedTokenSource;

/// Builder object for the IoT Hub Client
#[derive(Debug, Default)]
pub struct IoTHubClientBuilder<T> {
    iothub_hostname: Option<String>,
    device_id: Option<String>,
    token_source: Option<TokenProvider>,
    _phantom: PhantomData<T>,
}

impl Default for IoTHubClientBuilder<IoTHubClientBuilderUninitializedHubDetails> {
    fn default() -> IoTHubClientBuilder<IoTHubClientBuilderUninitializedHubDetails> {
        Self {
            iothub_hostname: None,
            device_id: None,
            token_source: None,
            _phantom: Default::default(),
        }
    }
}

impl IoTHubClientBuilder<IoTHubClientBuilderInitializedTokenSource> {
    /// Build the IoT hub client
    pub async fn build(self) -> Result<IoTHubClient, IoTHubClientBuilderError> {
        let Some(iothub_hostname) = self.iothub_hostname else {
            return Err(IoTHubClientBuilderError::UninitializedField(
                "iothub_hostname",
            ));
        };

        let Some(device_id) = self.device_id else {
            return Err(IoTHubClientBuilderError::UninitializedField("device_id"));
        };

        let Some(token_source) = self.token_source else {
            return Err(IoTHubClientBuilderError::ValidationError(
                "One of `access_key` must be provided such that the token source can be configured",
            ));
        };

        let client = IoTHubClient::new(iothub_hostname, device_id, token_source)
            .await
            .map_err(|e| IoTHubClientBuilderError::ClientError(format!("{e}")))?;

        Ok(client)
    }
}

impl IoTHubClientBuilder<IoTHubClientBuilderUninitializedHubDetails> {
    /// Set the values for the IoT hub hostname and device id
    pub fn iothub_details<T>(
        self,
        iothub_hostname: T,
        device_id: T,
    ) -> IoTHubClientBuilder<IoTHubClientBuilderInitializedHubDetails>
    where
        T: ToString,
    {
        IoTHubClientBuilder {
            iothub_hostname: Some(iothub_hostname.to_string()),
            device_id: Some(device_id.to_string()),
            token_source: None,
            _phantom: Default::default(),
        }
    }

    /// Set the values for the IoT hub hostname and device id and access key from the connection
    /// string
    pub fn connection_string<T>(
        self,
        connection_string: T,
    ) -> Result<IoTHubClientBuilder<IoTHubClientBuilderInitializedTokenSource>, TokenError>
    where
        T: AsRef<str>,
    {
        let (hub_name, device_id, token) = parse_connection_string(&connection_string)?;
        let token_source = DeviceKeyTokenSource::new(&hub_name, &device_id, &token)?;

        Ok(IoTHubClientBuilder {
            iothub_hostname: Some(hub_name.to_string()),
            device_id: Some(device_id.to_string()),
            token_source: Some(token_source.into()),
            _phantom: Default::default(),
        })
    }
}

impl IoTHubClientBuilder<IoTHubClientBuilderInitializedHubDetails> {
    /// The the device access key
    pub fn access_key<T>(
        self,
        access_key: T,
    ) -> Result<IoTHubClientBuilder<IoTHubClientBuilderInitializedTokenSource>, TokenError>
    where
        T: ToString + AsRef<[u8]>,
    {
        let token_source = DeviceKeyTokenSource::new(
            &self.iothub_hostname.as_ref().unwrap(),
            &self.device_id.as_ref().unwrap(),
            &access_key,
        )?;

        Ok(IoTHubClientBuilder {
            iothub_hostname: self.iothub_hostname,
            device_id: self.device_id,
            token_source: Some(token_source.into()),
            _phantom: Default::default(),
        })
    }
}
