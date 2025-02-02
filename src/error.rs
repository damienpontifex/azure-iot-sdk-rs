use thiserror::Error;

///
#[derive(Debug, Error)]
pub enum IoTHubError {
    #[error("")]
    IoError(#[from] std::io::Error),
    #[error("")]
    TlsError(#[from] native_tls::Error),
    #[error("{0}")]
    Other(String),
}
