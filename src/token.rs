use std::time::SystemTime;

use enum_dispatch::enum_dispatch;
use hmac::{Hmac, Mac};
use sha2::Sha256;

const DEVICEID_KEY: &str = "DeviceId";
const HOSTNAME_KEY: &str = "HostName";
const SHAREDACCESSKEY_KEY: &str = "SharedAccessKey";

///
#[derive(Debug)]
pub enum TokenError {
    ///
    ConnectionStringMissingRequiredParameter(&'static str),
    ///
    InvalidKeyFormat,
    ///
    InvalidKeyLength,
}

/// Provider for authentication
#[enum_dispatch]
pub trait TokenSource {
    /// Get the authentication value from the source
    fn get(&self, expiry: &SystemTime) -> String;
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
    fn get(&self, _: &SystemTime) -> String {
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
    pub fn new(hub: &str, device_id: &str, key: &str) -> Result<DeviceKeyTokenSource, TokenError> {
        // Verify key is base64
        let b64_key = base64::decode(&key).map_err(|_| TokenError::InvalidKeyFormat)?;
        // Verify key is the right length for Hmac
        Hmac::<Sha256>::new_from_slice(&b64_key).map_err(|_| TokenError::InvalidKeyLength)?;

        Ok(DeviceKeyTokenSource {
            resource_uri: format!("{}%2Fdevices%2F{}", hub, device_id),
            key: key.to_string(),
        })
    }

    /// Make the source from a connection string for the device
    pub fn new_from_connection_string(
        connection_string: &str,
    ) -> Result<DeviceKeyTokenSource, TokenError> {
        let mut key = None;
        let mut device_id = None;
        let mut hub = None;

        let parts: Vec<&str> = connection_string.split(';').collect();
        for p in parts {
            let s: Vec<&str> = p.split('=').collect();
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

        Self::new(hub, device_id, key)
    }
}

impl TokenSource for DeviceKeyTokenSource {
    fn get(&self, expiry: &SystemTime) -> String {
        let expiry_timestamp = expiry.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_secs();

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
    fn get(&self, _expiry: &SystemTime) -> String {
        self.password.clone()
    }
}

pub(crate) fn generate_token(key: &str, message: &str) -> String {
    // Checked base64 and hmac in new so should be safe to unwrap here
    let key = base64::decode(&key).unwrap();
    let mut mac = Hmac::<Sha256>::new_from_slice(&key).unwrap();
    mac.update(message.as_bytes());
    let mac_result = mac.finalize();
    let signature = base64::encode(mac_result.into_bytes());

    let pairs = &vec![("sig", signature)];
    serde_urlencoded::to_string(pairs).unwrap()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn test_sas_source() {
        // Should pass value through unchanged
        let sas_token_source = SasTokenSource::new("MySasString");
        assert_eq!(sas_token_source.get(&SystemTime::now()), "MySasString".to_string());
    }

    #[test]
    fn test_key_source() {
        let key_source = DeviceKeyTokenSource::new(
            "pontifex.azure-devices.net",
            "FirstDevice",
            "O+H9VTcdJP0TQkl7bh4nVG0OJNrEataMpuWB54D0VEc=",
        )
        .unwrap();
        let expiry = SystemTime::UNIX_EPOCH + Duration::from_secs(1593353305);
        assert_eq!(key_source.get(&expiry), "SharedAccessSignature sr=pontifex.azure-devices.net%2Fdevices%2FFirstDevice&sig=CKYVArtLm72J2UNWLb4V3XqPc679Ig3LX83G3nPExUc%3D&se=1593353305");
    }
}
