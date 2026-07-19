use crate::run::error::{RunError, RunErrorKind};

pub(crate) const MAX_TIMESTAMP_BYTES: usize = 64;

pub(crate) fn validate_reason(reason: &str) -> Result<String, RunError> {
    if is_machine_code(reason) {
        Ok(reason.to_owned())
    } else {
        Err(RunError::new(RunErrorKind::InvalidReason))
    }
}

pub(crate) fn validate_timestamp(at: &str) -> Result<String, RunError> {
    if at.is_empty() || at.len() > MAX_TIMESTAMP_BYTES || at.chars().any(char::is_control) {
        Err(RunError::new(RunErrorKind::InvalidTimestamp))
    } else {
        Ok(at.to_owned())
    }
}

pub(crate) fn is_machine_code(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty() || bytes.len() > 100 || !bytes[0].is_ascii_lowercase() {
        return false;
    }
    let mut previous_separator = false;
    for &byte in bytes {
        if matches!(byte, b'.' | b'_' | b'-') {
            if previous_separator {
                return false;
            }
            previous_separator = true;
        } else if byte.is_ascii_lowercase() || byte.is_ascii_digit() {
            previous_separator = false;
        } else {
            return false;
        }
    }
    !previous_separator
}

pub(crate) fn is_portable_identifier(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty() || bytes.len() > 100 || value.contains("..") {
        return false;
    }
    let boundary = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit();
    boundary(bytes[0])
        && boundary(bytes[bytes.len() - 1])
        && bytes
            .iter()
            .all(|&byte| boundary(byte) || matches!(byte, b'.' | b'_' | b'-'))
}
