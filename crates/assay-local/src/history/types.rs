//! Public domain types for the local history store.
//!
//! `StoredRecord` is the immutable snapshot a consumer reads; `RecordStatus`
//! is the lifecycle derived from the journal; `HistoryError` is the
//! non-sensitive failure type.

use serde::Serialize;
use serde_json::Value;

/// Lifecycle status of a stored record derived from the journal.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordStatus {
    Active,
    Deleted,
    Purged,
}

/// Stored, immutable analysis record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredRecord {
    pub(super) id: String,
    pub(super) sequence: u64,
    pub(super) recorded_at: String,
    pub(super) repository_id: Option<String>,
    pub(super) status: RecordStatus,
    pub(super) report: Value,
}

impl StoredRecord {
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Monotonic append sequence.
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn recorded_at(&self) -> &str {
        &self.recorded_at
    }

    /// Local repository identifier, if the report carried one.
    pub fn repository_id(&self) -> Option<&str> {
        self.repository_id.as_deref()
    }

    pub const fn status(&self) -> RecordStatus {
        self.status
    }

    pub const fn report(&self) -> &Value {
        &self.report
    }
}

/// Non-sensitive history-store failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HistoryError {
    pub(super) reason: &'static str,
}

impl HistoryError {
    pub const fn reason(self) -> &'static str {
        self.reason
    }
}

impl std::fmt::Display for HistoryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "history store error: {}", self.reason)
    }
}

impl std::error::Error for HistoryError {}