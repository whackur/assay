use crate::run::error::{RunError, RunErrorKind};
use crate::run::validation::is_portable_identifier;

/// A portable, non-path run identifier.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RunId(String);

impl RunId {
    /// Validates a canonical run identifier that cannot encode a filesystem path.
    pub fn new(value: &str) -> Result<Self, RunError> {
        if is_portable_identifier(value) {
            Ok(Self(value.to_owned()))
        } else {
            Err(RunError::new(RunErrorKind::InvalidRunId))
        }
    }

    /// Returns the canonical serialized identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
