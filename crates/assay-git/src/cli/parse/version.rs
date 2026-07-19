use crate::{CollectionError, CollectionStage};

use super::single_line;
use crate::cli::error::incompatible_git;

const MINIMUM_GIT_MAJOR: u64 = 2;
const MINIMUM_GIT_MINOR: u64 = 47;

pub(crate) fn parse_version(output: &[u8]) -> Result<String, CollectionError> {
    let line = std::str::from_utf8(single_line(output, CollectionStage::ProbeCapabilities)?)
        .map_err(|_| incompatible_git())?;
    let version = line
        .strip_prefix("git version ")
        .ok_or_else(incompatible_git)?;
    if version.is_empty() || version.len() > 80 || version.trim() != version {
        return Err(incompatible_git());
    }
    if !version.bytes().all(|byte| {
        byte.is_ascii_alphanumeric()
            || matches!(byte, b' ' | b'.' | b'-' | b'_' | b'(' | b')' | b'+')
    }) {
        return Err(incompatible_git());
    }
    let numeric_end = version
        .bytes()
        .take_while(|byte| byte.is_ascii_digit() || *byte == b'.')
        .count();
    let numeric = version[..numeric_end].trim_end_matches('.');
    let suffix = &version[numeric.len()..];
    if numeric
        .split('.')
        .any(|part| part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return Err(incompatible_git());
    }
    let dotted_suffix = suffix.strip_prefix('.').is_some_and(|value| {
        !value.is_empty()
            && value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'+')
            })
    });
    let parenthesized_suffix = suffix
        .strip_prefix(" (")
        .and_then(|value| value.strip_suffix(')'))
        .is_some_and(|value| {
            !value.is_empty()
                && value.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b' ' | b'.' | b'-' | b'_' | b'+')
                })
        });
    if !suffix.is_empty() && !dotted_suffix && !parenthesized_suffix {
        return Err(incompatible_git());
    }
    let mut parts = numeric.split('.');
    let major = parts
        .next()
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or_else(incompatible_git)?;
    let minor = parts
        .next()
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or_else(incompatible_git)?;
    if major < MINIMUM_GIT_MAJOR || (major == MINIMUM_GIT_MAJOR && minor < MINIMUM_GIT_MINOR) {
        return Err(incompatible_git());
    }
    Ok(version.to_owned())
}
