use serde::export::Formatter;

#[derive(Debug)]
/// Errors that can be raised during provisioning or registration
pub enum ErrorKind {
    /// The Azure provisioning portal rejected your request for the device
    AzureProvisioningRejectedRequest,
    /// The provisioning service replied, but there was no operation Id
    ProvisionRequestReplyMissingOperationId,
    /// Timed out trying to get the IoT Hub for your device ID
    FailedToGetIotHub,
}

impl std::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("{:?}", self))
    }
}

impl std::error::Error for ErrorKind {}