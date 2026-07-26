//! Private clone/fetch transport boundary.
//!
//! Real GitHub networking is out of scope for the local slice. The request
//! record structurally cannot hold a token, and the transport receives the
//! [`SecretToken`] as a separate argument so it never enters the recorded
//! request, its logs, or its results.

use serde::Serialize;

use crate::token::SecretToken;

/// Clone or fetch request for a remote private repository.
/// Token is deliberately absent and passed to the transport separately, so
/// serializing or logging a request cannot leak credential material.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PrivateFetchRequest {
    repository: String,
    revision: String,
}

impl PrivateFetchRequest {
    pub fn new(repository: impl Into<String>, revision: impl Into<String>) -> Self {
        Self {
            repository: repository.into(),
            revision: revision.into(),
        }
    }

    pub fn repository(&self) -> &str {
        &self.repository
    }

    pub fn revision(&self) -> &str {
        &self.revision
    }
}

/// Non-sensitive transport failure that never echoes credential material.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransportError {
    code: &'static str,
}

impl TransportError {
    pub const fn new(code: &'static str) -> Self {
        Self { code }
    }

    pub const fn code(self) -> &'static str {
        self.code
    }
}

/// Result of a completed private fetch, holding no credential material.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FetchOutcome {
    revision: String,
}

impl FetchOutcome {
    pub fn new(revision: impl Into<String>) -> Self {
        Self {
            revision: revision.into(),
        }
    }

    pub fn revision(&self) -> &str {
        &self.revision
    }
}

/// Fetches a private repository. The authorization token is received here and
/// nowhere else; implementations must not persist or log its value.
pub trait PrivateGitTransport {
    /// Uses `authorization` only for the wire request.
    fn fetch(
        &self,
        request: &PrivateFetchRequest,
        authorization: Option<&SecretToken>,
    ) -> Result<FetchOutcome, TransportError>;
}