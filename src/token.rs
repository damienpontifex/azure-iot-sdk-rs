use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use sha2::Sha256;

const DEVICEID_KEY: &str = "DeviceId";
const HOSTNAME_KEY: &str = "HostName";
const SHAREDACCESSKEY_KEY: &str = "SharedAccessKey";

///
pub trait TokenSource {
    ///
    fn get(&self, expiry: &DateTime<Utc>) -> String;
}

impl std::fmt::Debug for dyn TokenSource {
    fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        unimplemented!("Only implemented by implementations of TokenSource")
    }
}

///
#[derive(Debug, Clone)]
pub struct SasTokenSource<'a> {
    sas: &'a str,
}

impl<'a> SasTokenSource<'_> {
    ///
    pub fn new(sas: &'a str) -> SasTokenSource<'_> {
        SasTokenSource { sas }
    }
}

impl<'a> TokenSource for SasTokenSource<'_> {
    fn get(&self, _: &DateTime<Utc>) -> String {
        self.sas.to_string()
    }
}

///
#[derive(Debug, Clone)]
pub struct DeviceKeyTokenSource<'a> {
    resource_uri: String,
    device_id: &'a str,
    key: &'a str,
}

impl<'a> DeviceKeyTokenSource<'_> {
    ///
    pub fn new(hub: &str, device_id: &'a str, key: &'a str) -> DeviceKeyTokenSource<'a> {
        DeviceKeyTokenSource {
            resource_uri: format!("{}%2Fdevices%2F{}", hub, device_id),
            device_id,
            key,
        }
    }

    ///
    pub fn new_from_connection_string(connection_string: &'a str) -> DeviceKeyTokenSource<'a> {
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

        // let key = key.ok_or(ErrorKind::ConnectionStringMissingRequiredParameter(
        //     SHAREDACCESSKEY_KEY,
        // ))?;
        Self::new(hub.unwrap(), device_id.unwrap(), key.unwrap())
    }
}

impl<'a> TokenSource for DeviceKeyTokenSource<'_> {
    fn get(&self, expiry: &DateTime<Utc>) -> String {
        let expiry_timestamp = expiry.timestamp();

        let to_sign = format!("{}\n{}", &self.resource_uri, expiry_timestamp);

        let token = generate_token(self.key, &to_sign);

        let sas = format!(
            "SharedAccessSignature sr={}&{}&se={}",
            self.resource_uri, token, expiry_timestamp
        );

        sas
    }
}

pub(crate) fn generate_token(key: &str, message: &str) -> String {
    let key = base64::decode(&key).unwrap();
    let mut mac = Hmac::<Sha256>::new_varkey(&key).unwrap();
    mac.input(message.as_bytes());
    let mac_result = mac.result().code();
    let signature = base64::encode(mac_result.as_ref());

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
        );
        let expiry = Utc.ymd(2020, 6, 28).and_hms(14, 08, 25);
        assert_eq!(key_source.get(&expiry), "SharedAccessSignature sr=pontifex.azure-devices.net%2Fdevices%2FFirstDevice&sig=CKYVArtLm72J2UNWLb4V3XqPc679Ig3LX83G3nPExUc%3D&se=1593353305");
    }

    // #[test]
    // fn test_add() {
    //     assert_eq!(generate_sas("myiothub.azure-devices.net", "FirstDevice", "O+H9VTcdJP0Tqkl7bh4nVG0OJNrAataMpuWB54D0VEc=", 1_587_123_309), "SharedAccessSignature sr=myiothub.azure-devices.net%2Fdevices%2FFirstDevice&sig=vn0%2BgyIUKgaBhEU0ypyOhJ0gPK5fSY1TKdvcJ1HxhnQ%3D&se=1587123309".to_string());
    // }
}
