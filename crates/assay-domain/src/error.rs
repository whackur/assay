use std::{error::Error, fmt};

/// A validation error that never echoes the rejected value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DomainValueError {
    value_kind: &'static str,
    reason: &'static str,
}

impl DomainValueError {
    pub(crate) fn new(value_kind: &'static str, reason: &'static str) -> Self {
        Self { value_kind, reason }
    }

    /// Returns the stable name of the rejected value type.
    pub const fn value_kind(&self) -> &'static str {
        self.value_kind
    }

    /// Returns a non-sensitive validation reason.
    pub const fn reason(&self) -> &'static str {
        self.reason
    }
}

impl fmt::Display for DomainValueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid {}: {}", self.value_kind, self.reason)
    }
}

impl Error for DomainValueError {}

/// Generates a validated newtype around `String` with serde and string conversions.
macro_rules! validated_string_value {
    ($name:ident, $kind:literal, $validator:ident) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(String);

        impl $name {
            /// Returns the canonical serialized value.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl std::str::FromStr for $name {
            type Err = crate::DomainValueError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                $validator(value).map_err(|reason| crate::DomainValueError::new($kind, reason))?;
                Ok(Self(value.to_owned()))
            }
        }

        impl std::convert::TryFrom<String> for $name {
            type Error = crate::DomainValueError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                $validator(&value).map_err(|reason| crate::DomainValueError::new($kind, reason))?;
                Ok(Self(value))
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.serialize_str(&self.0)
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::try_from(value).map_err(serde::de::Error::custom)
            }
        }
    };
}

pub(crate) use validated_string_value;

pub(crate) const SHA256_PREFIX: &str = "sha256:";
pub(crate) const SHA256_HEX_LENGTH: usize = 64;
pub(crate) const GIT_SHA1_LENGTH: usize = 40;
pub(crate) const GIT_SHA256_LENGTH: usize = 64;
pub(crate) const MAX_COMPONENT_LENGTH: usize = 100;
pub(crate) const MAX_CODE_LENGTH: usize = 64;

pub(crate) fn is_safe_component(value: &str) -> bool {
    if value.is_empty() || value.len() > MAX_COMPONENT_LENGTH || value.contains("..") {
        return false;
    }
    let bytes = value.as_bytes();
    let is_boundary = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit();
    is_boundary(bytes[0])
        && is_boundary(bytes[bytes.len() - 1])
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}

pub(crate) fn is_lower_hex(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}
