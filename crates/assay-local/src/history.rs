//! Immutable, file-based local analysis history.
//!
//! Each analysis appends a new immutable record; a rescan never overwrites a
//! prior snapshot. Soft deletion, restoration, and purge are append-only
//! journal operations reserved for the local administrator. There is no
//! database: records are JSON files under a history root directory.
//!
//! The module is split by responsibility: [`admin`] owns the operator
//! capability token, [`types`] owns the public domain types, [`journal`] owns
//! the append-only journal, and [`store`] owns the file-based record store.

mod admin;
mod journal;
mod store;
mod types;

pub use admin::LocalAdministrator;
pub use store::LocalHistoryStore;
pub use types::{HistoryError, RecordStatus, StoredRecord};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
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

    fn record_path(root: &std::path::Path, sequence: u64) -> std::path::PathBuf {
        root.join("records").join(format!("{sequence:06}.json"))
    }
}
