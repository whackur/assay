use std::time::Duration;

/// Explicit resource limits applied to every installed-Git collection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CollectionLimits {
    pub command_timeout: Duration,
    pub max_stdout_bytes: usize,
    /// Before a child is treated as over limit.
    pub max_stderr_bytes: usize,
    pub max_tree_entries: usize,
    /// Filesystem entries inspected while validating the object store.
    pub max_object_store_entries: usize,
    /// Bytes read and hashed from one blob.
    pub max_object_bytes: u64,
    /// Reachable commits reported by the bounded history scan.
    pub max_history_commits: usize,
    /// Changed entries considered for parent rename detection.
    pub max_rename_candidates: usize,
}

impl Default for CollectionLimits {
    fn default() -> Self {
        Self {
            command_timeout: Duration::from_secs(10),
            max_stdout_bytes: 8 * 1024 * 1024,
            max_stderr_bytes: 64 * 1024,
            max_tree_entries: 100_000,
            max_object_store_entries: 1_000_000,
            max_object_bytes: 16 * 1024 * 1024,
            max_history_commits: 100_000,
            max_rename_candidates: 1_000,
        }
    }
}

impl CollectionLimits {
    pub(crate) fn is_valid(self) -> bool {
        !self.command_timeout.is_zero()
            && self.max_stdout_bytes > 0
            && self.max_stderr_bytes > 0
            && self.max_tree_entries > 0
            && self.max_object_store_entries > 0
            && self.max_object_bytes > 0
            && self.max_history_commits > 0
            && self.max_rename_candidates > 0
    }
}
