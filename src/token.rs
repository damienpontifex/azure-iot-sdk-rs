use base64::prelude::{Engine as _, BASE64_STANDARD};
use chrono::{DateTime, Utc};
use enum_dispatch::enum_dispatch;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use thiserror::Error;

const DEVICEID_KEY: &str = "DeviceId";
const HOSTNAME_KEY: &str = "HostName";
const SHAREDACCESSKEY_KEY: &str = "SharedAccessKey";

///
#[derive(Debug, Error)]
pub enum TokenError {
    ///
    #[error("Connection string was missing required fields. All of `DeviceId`, `HostName`, and `SharedAccessKey` must be present.")]
    ConnectionStringMissingRequiredParameter(&'static str),
    ///
    #[error("The access key provided was invalid and failed to be parsed")]
    InvalidKeyFormat,
    ///
    #[error("The access key provided was invalid length and failed to be parsed")]
    InvalidKeyLength,
}

/// Provider for authentication
#[enum_dispatch]
pub trait TokenSource {
    /// Get the authentication value from the source
    fn get(&self, expiry: &DateTime<Utc>) -> String;
}

#[enum_dispatch(TokenSource)]
#[derive(Debug, Clone)]
pub enum TokenProvider {
    SasTokenSource,
    DeviceKeyTokenSource,
    UsernamePasswordTokenSource,
}

/// Provide an existing SAS as authentication source
#[derive(Debug, Clone)]
pub struct SasTokenSource {
    sas: String,
}

impl SasTokenSource {
    /// Make the token source with the given SAS
    pub fn new<T>(sas: T) -> SasTokenSource
    where
        T: ToString,
    {
        SasTokenSource {
            sas: sas.to_string(),
        }
    }
}

impl TokenSource for SasTokenSource {
    fn get(&self, _: &DateTime<Utc>) -> String {
        self.sas.clone()
    }
}

/// Authenticate using the devices key
#[derive(Debug, Clone)]
pub struct DeviceKeyTokenSource {
    resource_uri: String,
    key: String,
}

impl DeviceKeyTokenSource {
    /// Make the source from the devices individual details
    pub fn new<R, T>(hub: &R, device_id: &R, key: &T) -> Result<DeviceKeyTokenSource, TokenError>
    where
        R: AsRef<str> + ?Sized,
        T: ToString + AsRef<[u8]> + ?Sized,
    {
        // Verify key is base64
        let b64_key = BASE64_STANDARD
            .decode(&key)
            .map_err(|_| TokenError::InvalidKeyFormat)?;
        // Verify key is the right length for Hmac
        Hmac::<Sha256>::new_from_slice(&b64_key).map_err(|_| TokenError::InvalidKeyLength)?;

        Ok(DeviceKeyTokenSource {
            resource_uri: format!("{}%2Fdevices%2F{}", hub.as_ref(), device_id.as_ref()),
            key: key.to_string(),
        })
    }

    /// Make the source from a connection string for the device
    pub fn new_from_connection_string<T>(
        connection_string: &T,
    ) -> Result<DeviceKeyTokenSource, TokenError>
    where
        T: AsRef<str>,
    {
        let (hub, device_id, key) = parse_connection_string(&connection_string)?;

        Self::new(hub, device_id, key)
    }
}

pub(crate) fn parse_connection_string<'a, T>(
    connection_string: &'a T,
) -> Result<(&'a str, &'a str, &'a str), TokenError>
where
    T: AsRef<str>,
{
    let mut key = None;
    let mut device_id = None;
    let mut hub = None;

    let parts: Vec<_> = connection_string.as_ref().split(';').collect();
    for p in parts {
        let s: Vec<_> = p.split('=').collect();
        match s[0] {
            SHAREDACCESSKEY_KEY => key = Some(s[1]),
            DEVICEID_KEY => device_id = Some(s[1]),
            HOSTNAME_KEY => hub = Some(s[1]),
            _ => (), // Ignore extraneous component in the connection string
        }
    }

    let hub = hub.ok_or(TokenError::ConnectionStringMissingRequiredParameter(
        HOSTNAME_KEY,
    ))?;
    let device_id = device_id.ok_or(TokenError::ConnectionStringMissingRequiredParameter(
        DEVICEID_KEY,
    ))?;
    let key = key.ok_or(TokenError::ConnectionStringMissingRequiredParameter(
        SHAREDACCESSKEY_KEY,
    ))?;

    Ok((hub, device_id, key))
}

impl TokenSource for DeviceKeyTokenSource {
    fn get(&self, expiry: &DateTime<Utc>) -> String {
        let expiry_timestamp = expiry.timestamp();

        let to_sign = format!("{}\n{}", &self.resource_uri, expiry_timestamp);

        let token = generate_token(&self.key, &to_sign);

        let sas = format!(
            "SharedAccessSignature sr={}&{}&se={}",
            self.resource_uri, token, expiry_timestamp
        );

        sas
    }
}

/// Useful if you need to customize the user name and password sent to Azure (for example
/// to implement an IoT Plug-n-Play device))
#[derive(Debug, Clone)]
pub struct UsernamePasswordTokenSource {
    username: String,
    password: String,
}

impl TokenSource for UsernamePasswordTokenSource {
    fn get(&self, _expiry: &DateTime<Utc>) -> String {
        self.password.clone()
    }
}

pub(crate) fn generate_token(key: &str, message: &str) -> String {
    // Checked base64 and hmac in new so should be safe to unwrap here
    let key = BASE64_STANDARD.decode(&key).unwrap();
    let mut mac = Hmac::<Sha256>::new_from_slice(&key).unwrap();
    mac.update(message.as_bytes());
    let mac_result = mac.finalize();
    let signature = BASE64_STANDARD.encode(mac_result.into_bytes());

    let pairs = &vec![("sig", signature)];
    serde_urlencoded::to_string(pairs).unwrap()
}

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;
    use chrono::prelude::*;

    #[test]
    fn test_sas_source() {
        // Should pass value through unchanged
        let sas_token_source = SasTokenSource::new("MySasString");
        assert_eq!(sas_token_source.get(&Utc::now()), "MySasString".to_string());
    }

    #[test]
    fn test_key_source() {
        let key_source = DeviceKeyTokenSource::new(
            "pontifex.azure-devices.net",
            "FirstDevice",
            "O+H9VTcdJP0TQkl7bh4nVG0OJNrEataMpuWB54D0VEc=",
        )
        .unwrap();
        let expiry = Utc.with_ymd_and_hms(2020, 6, 28, 14, 08, 25).unwrap();
        assert_eq!(key_source.get(&expiry), "SharedAccessSignature sr=pontifex.azure-devices.net%2Fdevices%2FFirstDevice&sig=CKYVArtLm72J2UNWLb4V3XqPc679Ig3LX83G3nPExUc%3D&se=1593353305");
    }
}
