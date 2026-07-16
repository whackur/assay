//! Private clone/fetch transport boundary.
//!
//! Real GitHub networking is out of scope for the local slice. This module
//! defines the seam: a request record that structurally cannot hold a token,
//! and a transport trait that receives the [`SecretToken`] as a separate
//! argument so it never enters the recorded request, its logs, or its results.

use serde::Serialize;

use crate::token::SecretToken;

/// A clone or fetch request for a remote private repository.
///
/// The token is deliberately absent: it is passed to the transport separately,
/// so serializing or logging a request cannot leak credential material.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PrivateFetchRequest {
    repository: String,
    revision: String,
}

impl PrivateFetchRequest {
    /// Builds a request from a canonical repository locator and revision.
    pub fn new(repository: impl Into<String>, revision: impl Into<String>) -> Self {
        Self {
            repository: repository.into(),
            revision: revision.into(),
        }
    }

    /// Returns the target repository locator.
    pub fn repository(&self) -> &str {
        &self.repository
    }

    /// Returns the requested revision selector.
    pub fn revision(&self) -> &str {
        &self.revision
    }
}

/// A non-sensitive transport failure that never echoes credential material.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransportError {
    code: &'static str,
}

impl TransportError {
    /// Builds a transport error from a machine-stable code.
    pub const fn new(code: &'static str) -> Self {
        Self { code }
    }

    /// Returns the machine-stable error code.
    pub const fn code(self) -> &'static str {
        self.code
    }
}

/// The result of a completed private fetch, holding no credential material.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FetchOutcome {
    revision: String,
}

impl FetchOutcome {
    /// Records the resolved immutable revision.
    pub fn new(revision: impl Into<String>) -> Self {
        Self {
            revision: revision.into(),
        }
    }

    /// Returns the resolved revision.
    pub fn revision(&self) -> &str {
        &self.revision
    }
}

/// Fetches a private repository. The authorization token is received here and
/// nowhere else; implementations must not persist or log its value.
pub trait PrivateGitTransport {
    /// Performs the fetch, using `authorization` only for the wire request.
    fn fetch(
        &self,
        request: &PrivateFetchRequest,
        authorization: Option<&SecretToken>,
    ) -> Result<FetchOutcome, TransportError>;
}
