//! The append-only journal backing soft-delete, restore, and purge.
//!
//! Journal entries are JSON lines in `journal.log`. Replaying the journal
//! derives the lifecycle status of every record id; the store layer joins
//! that with present record files to produce [`StoredRecord`] listings.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::types::{HistoryError, RecordStatus};

#[derive(Serialize, Deserialize)]
pub(super) struct JournalEntry {
    pub(super) op: JournalOp,
    pub(super) id: String,
    pub(super) at: String,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum JournalOp {
    SoftDelete,
    Restore,
    Purge,
}

pub(super) fn journal_path(root: &Path) -> std::path::PathBuf {
    root.join("journal.log")
}

pub(super) fn append_journal(
    root: &Path,
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
        .open(journal_path(root))
        .map_err(|_| HistoryError {
            reason: "cannot open journal",
        })?;
    file.write_all(line.as_bytes()).map_err(|_| HistoryError {
        reason: "cannot append journal entry",
    })
}

pub(super) fn journal_statuses(root: &Path) -> Result<Vec<(String, RecordStatus)>, HistoryError> {
    let path = journal_path(root);
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
