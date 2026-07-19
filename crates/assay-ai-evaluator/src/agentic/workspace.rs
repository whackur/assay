use std::path::{Path, PathBuf};

use super::instructions::ControlInputs;

/// Prepared execution locations for one agent run: an immutable read-only
/// snapshot of the analyzed revision, a writable control directory holding
/// the task inputs, and the one designated judgment output path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedWorkspace {
    snapshot_dir: PathBuf,
    control_dir: PathBuf,
    output_path: PathBuf,
}

impl PreparedWorkspace {
    /// Records the prepared locations returned by a workspace port.
    pub const fn new(snapshot_dir: PathBuf, control_dir: PathBuf, output_path: PathBuf) -> Self {
        Self {
            snapshot_dir,
            control_dir,
            output_path,
        }
    }

    /// Returns the immutable snapshot tree the agent is constrained to.
    pub fn snapshot_dir(&self) -> &Path {
        &self.snapshot_dir
    }

    /// Returns the only writable location available to the agent.
    pub fn control_dir(&self) -> &Path {
        &self.control_dir
    }

    /// Returns the designated path the agent must write its judgment to.
    pub fn output_path(&self) -> &Path {
        &self.output_path
    }
}

/// Redacted snapshot-workspace failure with no path or repository text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceError {
    /// The analyzed commit could not be materialized as a snapshot tree.
    SnapshotUnavailable,
    /// The control directory or task inputs could not be prepared.
    ControlWriteFailed,
}

/// Snapshot materialization seam. The concrete Git-backed implementation
/// lives in the deployment layer, exactly as `HttpTransport` does.
pub trait SnapshotWorkspace {
    /// Materializes the analyzed commit as an immutable read-only snapshot
    /// plus a writable control directory seeded with the task inputs, and
    /// returns the prepared locations.
    fn materialize(&self, inputs: &ControlInputs<'_>) -> Result<PreparedWorkspace, WorkspaceError>;

    /// Disposes of one prepared workspace after the run. Cleanup is
    /// best-effort and never alters an already-collected judgment.
    fn dispose(&self, workspace: PreparedWorkspace);
}
