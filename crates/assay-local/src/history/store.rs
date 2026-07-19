//! The append-only, file-based store of immutable local analysis records.
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

use super::admin::LocalAdministrator;
use super::journal::{JournalOp, append_journal, journal_statuses};
use super::types::{HistoryError, RecordStatus, StoredRecord};

#[derive(Serialize, Deserialize)]
struct RecordFile {
    id: String,
    sequence: u64,
    recorded_at: String,
    repository_id: Option<String>,
    report: Value,
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
        let statuses = journal_statuses(&self.root)?;
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
        append_journal(&self.root, JournalOp::SoftDelete, id, at)
    }

    /// Restores a soft-deleted record. Administrator-only.
    pub fn restore(
        &self,
        id: &str,
        _operator: &LocalAdministrator,
        at: impl Into<String>,
    ) -> Result<(), HistoryError> {
        append_journal(&self.root, JournalOp::Restore, id, at)
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
        append_journal(&self.root, JournalOp::Purge, id, at)
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
