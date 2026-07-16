//! Local private-repository analysis, history, consent, and loopback dashboard.
//!
//! This crate owns the local single-operator surface: loopback-only binding, an
//! immutable file-based history of local reports, the private-feature consent
//! and section-status model, and the named-environment-variable token boundary.
//! It performs no hosted networking and never publishes to the public catalog
//! or comparison corpus. GitHub PAT values reach only the transport seam and
//! never a command argument, log, result, error, or stored record.

#![forbid(unsafe_code)]

mod consent;
mod history;
mod loopback;
mod report;
mod serve;
mod token;
mod transport;

pub use consent::{
    ConsentGrant, ConsentState, ExternalProvider, ExternalTransmission, NextAction, PrivateFeature,
    SectionReason, SectionReport, SectionState,
};
pub use history::{
    HistoryError, LocalAdministrator, LocalHistoryStore, RecordStatus, StoredRecord,
};
pub use loopback::LoopbackListener;
pub use report::{LOCAL_REPORT_SCHEMA_VERSION, LocalReport, LocalReportError};
pub use serve::{HttpResponse, handle_request, run, serve_connection, serve_next};
pub use token::{
    GithubTokenEnvVar, MapEnvironment, ProcessEnvironment, SecretToken, TokenEnvVarError,
    TokenEnvironment, TokenResolutionError, resolve_token,
};
pub use transport::{FetchOutcome, PrivateFetchRequest, PrivateGitTransport, TransportError};

/// Stable package identifier for diagnostics and capability reporting.
pub const PACKAGE_NAME: &str = env!("CARGO_PKG_NAME");
