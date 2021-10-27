//! Digital Twin Model Identifier
//!
//! See https://github.com/Azure/digital-twin-model-identifier

use std::error::Error;
use std::fmt;
use std::str::FromStr;

/// A wrapper struct around a string.
///
/// A DTMI is a string on the form "DTMI:<path>;<version> where <path> consists of colon-separated
/// segments. Each segment is a non-empty string containing only letters, digits, and underscores.
/// The version number is limited to 9 digits, the first digit may not be zero.
///
/// See https://github.com/Azure/digital-twin-model-identifier
#[derive(Debug)]
pub struct Dtmi(pub(crate) String);

/// Returned when a given string cannot be validated as a DTMI.
#[derive(Debug)]
pub struct DtmiValidationError;

impl fmt::Display for DtmiValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Input was not valid DTMI")
    }
}

impl Error for DtmiValidationError {}

impl FromStr for Dtmi {
    type Err = DtmiValidationError;
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        if checkdtmi(input) {
            Ok(Dtmi(input.into()))
        } else {
            Err(DtmiValidationError)
        }
    }
}

/// Implements the following regex
///
/// ^dtmi:[A-Za-z](:[A-Za-z0-9_]*[A-Za-z0-9])?(:[A-Za-z](:[A-Za-z0-9_]*[A-Za-z0-9])?)*;[1-9][0-9]{0,8}$
///
/// Note that unlike the C version provided in Azure's documentation, this version doesn't support
/// the sysok flag (so a leading underscore is not allowed for any path segments).
///
fn checkdtmi(id: &str) -> bool {
    if id.len() < 5 || !id.starts_with("dtmi:") {
        return false;
    }

    let mut pos = 4;
    let bid = id.as_bytes();
    let len = bid.len();

    macro_rules! incr_or_false {
        ($pos:ident, $len:ident) => {
            $pos += 1;
            if $pos == $len {
                return false;
            }
        };
    }

    while bid[pos] == b':' {
        incr_or_false!(pos, len);
        if !bid[pos].is_ascii_alphabetic() {
            return false;
        }

        incr_or_false!(pos, len);
        while bid[pos] != b':' && bid[pos] != b';' {
            if !(bid[pos].is_ascii_alphanumeric() || bid[pos] == b'_') {
                return false;
            }

            incr_or_false!(pos, len);
        }
        if bid[pos - 1] == b'_' {
            return false;
        }
    }

    // Last bit is version number: Must be at most 9 characters, all numeric, first number cannot
    // be zero.
    incr_or_false!(pos, len);
    if bid[pos..].len() > 9 {
        return false;
    }

    if bid[pos] < b'1' || bid[pos] > b'9' {
        return false;
    } // First number must not be zero

    pos += 1;
    while pos < len {
        if !bid[pos].is_ascii_digit() {
            return false;
        }
        pos += 1;
    }

    true
}

#[cfg(test)]
mod tests {
    #[test]
    fn dtmi_from_str() {
        let cases = vec![
            ("foo", false),
            ("dtmi:", false),
            ("dtmi:foo", false),
            ("dtmi:foo:", false),
            ("dtmi:foo;", false),
            ("dtmi:foo;f", false),
            ("dtmi:1b;f", false),
            ("dtmi:k;f", false),
            ("dtmi:foo_bar:u_16:baz33:qux;12", true),
            ("dtmi:com:microsoft:azure:iot:demoSensor5;1", true),
            ("dtmi:mb56228c18_d7ff_11eb_bd6b_5ac573913239;12", true),
            ("dtmi:mb56228c18_d7ff_11eb_bd6b_5ac573913239;12345", true),
            ("dtmi:mb56228c18_d7ff_11eb_bd6b_5ac573913239;123456", true),
            (
                "dtmi:mb56228c18_d7ff_11eb_bd6b_5ac573913239;123456789",
                true,
            ),
            (
                "dtmi:mb56228c18_d7ff_11eb_bd6b_5ac573913239;0123456789",
                false,
            ),
        ];

        for (input, should_be_accepted) in cases {
            let maybe_dtmi: Result<_, _> = input.parse::<super::Dtmi>();
            if should_be_accepted {
                assert!(
                    maybe_dtmi.is_ok(),
                    "input '{}' should be accepted, but it wasn't",
                    input
                );
            } else {
                assert!(
                    maybe_dtmi.is_err(),
                    "input '{}' should not be accepted, but it was",
                    input
                );
            }
        }
    }
}
