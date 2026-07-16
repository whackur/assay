//! Immutable, file-based local analysis history.
//!
//! Each analysis appends a new immutable record; a rescan never overwrites a
//! prior snapshot. Soft deletion, restoration, and purge are append-only
//! journal operations reserved for the local administrator. There is no
//! database: records are JSON files under a history root directory.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A capability held only by the single local operator (the local
/// administrator). Mutating history requires presenting this token, so soft
/// deletion, restoration, and purge cannot happen without operator authority.
#[derive(Clone, Copy, Debug)]
pub struct LocalAdministrator(());

impl LocalAdministrator {
    /// Assumes the local-operator role for the current process.
    pub const fn assume_local_operator() -> Self {
        Self(())
    }
}

/// The lifecycle status of a stored record derived from the journal.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordStatus {
    Active,
    Deleted,
    Purged,
}

/// A stored, immutable analysis record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredRecord {
    id: String,
    sequence: u64,
    recorded_at: String,
    repository_id: Option<String>,
    status: RecordStatus,
    report: Value,
}

impl StoredRecord {
    /// Returns the stable record identifier.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the monotonic append sequence.
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Returns the recorded timestamp.
    pub fn recorded_at(&self) -> &str {
        &self.recorded_at
    }

    /// Returns the local repository identifier, if the report carried one.
    pub fn repository_id(&self) -> Option<&str> {
        self.repository_id.as_deref()
    }

    /// Returns the current lifecycle status.
    pub const fn status(&self) -> RecordStatus {
        self.status
    }

    /// Returns the stored report payload.
    pub const fn report(&self) -> &Value {
        &self.report
    }
}

/// A non-sensitive history-store failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HistoryError {
    reason: &'static str,
}

impl HistoryError {
    /// Returns a machine-stable reason code.
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

#[derive(Serialize, Deserialize)]
struct RecordFile {
    id: String,
    sequence: u64,
    recorded_at: String,
    repository_id: Option<String>,
    report: Value,
}

#[derive(Serialize, Deserialize)]
struct JournalEntry {
    op: JournalOp,
    id: String,
    at: String,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum JournalOp {
    SoftDelete,
    Restore,
    Purge,
}

/// An append-only, file-based store of immutable local analysis records.
#[derive(Clone, Debug)]
pub struct LocalHistoryStore {
    root: PathBuf,
}

impl LocalHistoryStore {
    /// Opens or creates a history store rooted at `root`.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, HistoryError> {
        let root = root.into();
        fs::create_dir_all(records_dir(&root)).map_err(|_| HistoryError {
            reason: "cannot create records directory",
        })?;
        Ok(Self { root })
    }

    /// Appends a new immutable record and returns it. Never overwrites a prior
    /// snapshot.
    pub fn append(
        &self,
        report: Value,
        recorded_at: impl Into<String>,
    ) -> Result<StoredRecord, HistoryError> {
        let sequence = self.next_sequence()?;
        let id = record_id(sequence);
        let repository_id = report
            .get("repository")
            .and_then(|repository| repository.get("repository_id"))
            .and_then(Value::as_str)
            .map(str::to_owned);
        let recorded_at = recorded_at.into();
        let file = RecordFile {
            id: id.clone(),
            sequence,
            recorded_at: recorded_at.clone(),
            repository_id: repository_id.clone(),
            report: report.clone(),
        };
        let bytes = serde_json::to_vec_pretty(&file).map_err(|_| HistoryError {
            reason: "cannot serialize record",
        })?;
        write_new_file(&record_path(&self.root, sequence), &bytes)?;
        Ok(StoredRecord {
            id,
            sequence,
            recorded_at,
            repository_id,
            status: RecordStatus::Active,
            report,
        })
    }

    /// Lists active (not deleted or purged) records ordered by sequence.
    pub fn list_active(&self) -> Result<Vec<StoredRecord>, HistoryError> {
        Ok(self
            .list_all()?
            .into_iter()
            .filter(|record| record.status == RecordStatus::Active)
            .collect())
    }

    /// Lists all present records with their lifecycle status ordered by
    /// sequence. Purged records are absent because their content is removed.
    pub fn list_all(&self) -> Result<Vec<StoredRecord>, HistoryError> {
        let statuses = self.journal_statuses()?;
        let mut records = Vec::new();
        for sequence in self.present_sequences()? {
            let file = self.read_record(sequence)?;
            let status = statuses
                .iter()
                .find(|(id, _)| id == &file.id)
                .map_or(RecordStatus::Active, |(_, status)| *status);
            records.push(StoredRecord {
                id: file.id,
                sequence: file.sequence,
                recorded_at: file.recorded_at,
                repository_id: file.repository_id,
                status,
                report: file.report,
            });
        }
        records.sort_by_key(StoredRecord::sequence);
        Ok(records)
    }

    /// Returns an active record by identifier, if present.
    pub fn get_active(&self, id: &str) -> Result<Option<StoredRecord>, HistoryError> {
        Ok(self
            .list_active()?
            .into_iter()
            .find(|record| record.id == id))
    }

    /// Soft-deletes a record. Administrator-only; the record file is retained.
    pub fn soft_delete(
        &self,
        id: &str,
        _operator: &LocalAdministrator,
        at: impl Into<String>,
    ) -> Result<(), HistoryError> {
        self.append_journal(JournalOp::SoftDelete, id, at)
    }

    /// Restores a soft-deleted record. Administrator-only.
    pub fn restore(
        &self,
        id: &str,
        _operator: &LocalAdministrator,
        at: impl Into<String>,
    ) -> Result<(), HistoryError> {
        self.append_journal(JournalOp::Restore, id, at)
    }

    /// Purges a record, removing its content irrecoverably. Administrator-only.
    pub fn purge(
        &self,
        id: &str,
        _operator: &LocalAdministrator,
        at: impl Into<String>,
    ) -> Result<(), HistoryError> {
        let sequence = sequence_of(id).ok_or(HistoryError {
            reason: "malformed record id",
        })?;
        let path = record_path(&self.root, sequence);
        if path.exists() {
            fs::remove_file(&path).map_err(|_| HistoryError {
                reason: "cannot remove record file",
            })?;
        }
        self.append_journal(JournalOp::Purge, id, at)
    }

    fn append_journal(
        &self,
        op: JournalOp,
        id: &str,
        at: impl Into<String>,
    ) -> Result<(), HistoryError> {
        let entry = JournalEntry {
            op,
            id: id.to_owned(),
            at: at.into(),
        };
        let mut line = serde_json::to_string(&entry).map_err(|_| HistoryError {
            reason: "cannot serialize journal entry",
        })?;
        line.push('\n');
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(journal_path(&self.root))
            .map_err(|_| HistoryError {
                reason: "cannot open journal",
            })?;
        file.write_all(line.as_bytes()).map_err(|_| HistoryError {
            reason: "cannot append journal entry",
        })
    }

    fn journal_statuses(&self) -> Result<Vec<(String, RecordStatus)>, HistoryError> {
        let path = journal_path(&self.root);
        let mut statuses: Vec<(String, RecordStatus)> = Vec::new();
        let Ok(text) = fs::read_to_string(&path) else {
            return Ok(statuses);
        };
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            let entry: JournalEntry = serde_json::from_str(line).map_err(|_| HistoryError {
                reason: "corrupt journal entry",
            })?;
            let status = match entry.op {
                JournalOp::SoftDelete => RecordStatus::Deleted,
                JournalOp::Restore => RecordStatus::Active,
                JournalOp::Purge => RecordStatus::Purged,
            };
            if let Some(existing) = statuses.iter_mut().find(|(id, _)| id == &entry.id) {
                existing.1 = status;
            } else {
                statuses.push((entry.id, status));
            }
        }
        Ok(statuses)
    }

    fn next_sequence(&self) -> Result<u64, HistoryError> {
        Ok(self.present_sequences()?.into_iter().max().unwrap_or(0) + 1)
    }

    fn present_sequences(&self) -> Result<Vec<u64>, HistoryError> {
        let mut sequences = Vec::new();
        let entries = fs::read_dir(records_dir(&self.root)).map_err(|_| HistoryError {
            reason: "cannot read records directory",
        })?;
        for entry in entries {
            let entry = entry.map_err(|_| HistoryError {
                reason: "cannot read record entry",
            })?;
            if let Some(sequence) = entry
                .path()
                .file_stem()
                .and_then(|stem| stem.to_str())
                .and_then(|stem| stem.parse::<u64>().ok())
            {
                sequences.push(sequence);
            }
        }
        Ok(sequences)
    }

    fn read_record(&self, sequence: u64) -> Result<RecordFile, HistoryError> {
        let bytes = fs::read(record_path(&self.root, sequence)).map_err(|_| HistoryError {
            reason: "cannot read record file",
        })?;
        serde_json::from_slice(&bytes).map_err(|_| HistoryError {
            reason: "corrupt record file",
        })
    }
}

fn records_dir(root: &Path) -> PathBuf {
    root.join("records")
}

fn journal_path(root: &Path) -> PathBuf {
    root.join("journal.log")
}

fn record_path(root: &Path, sequence: u64) -> PathBuf {
    records_dir(root).join(format!("{sequence:06}.json"))
}

fn record_id(sequence: u64) -> String {
    format!("rec-{sequence:06}")
}

fn sequence_of(id: &str) -> Option<u64> {
    id.strip_prefix("rec-").and_then(|rest| rest.parse().ok())
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), HistoryError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| HistoryError {
            reason: "record already exists",
        })?;
    file.write_all(bytes).map_err(|_| HistoryError {
        reason: "cannot write record file",
    })?;
    file.sync_all().map_err(|_| HistoryError {
        reason: "cannot flush record file",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    fn report(marker: &str) -> Value {
        json!({ "repository": { "repository_id": "abc" }, "marker": marker })
    }

    #[test]
    fn append_accumulates_immutable_records_across_rescans() {
        let dir = TempDir::new().unwrap();
        let store = LocalHistoryStore::open(dir.path()).unwrap();
        let first = store
            .append(report("first"), "2026-07-16T00:00:00Z")
            .unwrap();
        let second = store
            .append(report("second"), "2026-07-16T01:00:00Z")
            .unwrap();
        assert_eq!(first.id(), "rec-000001");
        assert_eq!(second.sequence(), 2);
        let all = store.list_active().unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].report()["marker"], "first");
        assert_eq!(all[1].report()["marker"], "second");
    }

    #[test]
    fn soft_delete_restore_and_purge_are_operator_actions() {
        let dir = TempDir::new().unwrap();
        let store = LocalHistoryStore::open(dir.path()).unwrap();
        let record = store.append(report("only"), "t0").unwrap();
        let operator = LocalAdministrator::assume_local_operator();

        store.soft_delete(record.id(), &operator, "t1").unwrap();
        assert!(store.get_active(record.id()).unwrap().is_none());
        assert_eq!(store.list_all().unwrap()[0].status(), RecordStatus::Deleted);

        store.restore(record.id(), &operator, "t2").unwrap();
        assert!(store.get_active(record.id()).unwrap().is_some());

        store.purge(record.id(), &operator, "t3").unwrap();
        assert!(store.list_all().unwrap().is_empty());
        assert!(!record_path(dir.path(), record.sequence()).exists());
    }
}
