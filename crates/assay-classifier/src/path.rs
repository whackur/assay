//! Repository-relative portable path validation.
//!
//! Split from `lib.rs` so path validation rules stay separate from the
//! classification policy and identifier contracts that consume them.

use std::fmt;

use crate::error::ClassificationError;

/// A repository-relative UTF-8 path using `/` separators.
///
/// Absolute paths, traversal components, repeated separators, NUL bytes, and
/// platform-specific separators are rejected. Spaces, Unicode, and ASCII case
/// variants are preserved. Consumers should avoid logging this value because a
/// repository path can itself contain private information.
#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PortablePath(String);

impl PortablePath {
    /// Returns the validated repository-relative path.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn lowercase_components(&self) -> Vec<String> {
        self.0
            .split('/')
            .map(|component| component.to_ascii_lowercase())
            .collect()
    }
}

impl fmt::Debug for PortablePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PortablePath(<redacted>)")
    }
}

impl TryFrom<&str> for PortablePath {
    type Error = ClassificationError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        validate_portable_path(value)?;
        Ok(Self(value.to_owned()))
    }
}

impl TryFrom<String> for PortablePath {
    type Error = ClassificationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        validate_portable_path(&value)?;
        Ok(Self(value))
    }
}

fn validate_portable_path(value: &str) -> Result<(), ClassificationError> {
    if value.is_empty() {
        return Err(ClassificationError::portable_path(
            "expected a non-empty path",
        ));
    }
    if value.contains('\0') {
        return Err(ClassificationError::portable_path(
            "NUL bytes are not allowed",
        ));
    }
    if value.starts_with('/') || value.starts_with('\\') || has_windows_drive_prefix(value) {
        return Err(ClassificationError::portable_path(
            "absolute paths are not allowed",
        ));
    }
    if value.contains('\\') {
        return Err(ClassificationError::portable_path(
            "expected portable forward-slash separators",
        ));
    }
    if value
        .split('/')
        .any(|component| component.is_empty() || matches!(component, "." | ".."))
    {
        return Err(ClassificationError::portable_path(
            "empty and traversal components are not allowed",
        ));
    }
    Ok(())
}

fn has_windows_drive_prefix(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}
