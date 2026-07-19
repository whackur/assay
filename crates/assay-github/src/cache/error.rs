use std::{error::Error, fmt};

/// A non-sensitive cache contract validation error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CacheValueError {
    kind: &'static str,
    reason: &'static str,
}

impl CacheValueError {
    pub(crate) const fn new(kind: &'static str, reason: &'static str) -> Self {
        Self { kind, reason }
    }

    /// Returns the rejected value kind without returning the value.
    pub const fn kind(self) -> &'static str {
        self.kind
    }

    /// Returns a non-sensitive validation reason.
    pub const fn reason(self) -> &'static str {
        self.reason
    }
}

impl fmt::Display for CacheValueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid {}: {}", self.kind, self.reason)
    }
}

impl Error for CacheValueError {}
