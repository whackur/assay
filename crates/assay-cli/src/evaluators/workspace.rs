use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use assay_ai_evaluator::{ControlInputs, PreparedWorkspace, SnapshotWorkspace, WorkspaceError};

/// Git-backed [`SnapshotWorkspace`]: materializes the analyzed commit into a
/// temporary tree with the same trusted-executable discipline as ADR 0002.
///
/// The snapshot is a fresh clone checked out at the exact analyzed commit
/// with its `.git` directory removed, so the agent sees the exact tree of the
/// analyzed revision — never the operator's live working copy and never
/// repository history. The control directory receives the host-authored
/// instructions, the canonical request payload, and the mandatory evidence
/// list; the designated judgment output path lives inside it.
#[derive(Debug)]
pub struct GitSnapshotWorkspace {
    git: PathBuf,
    repository: PathBuf,
}

impl GitSnapshotWorkspace {
    /// Binds a trusted absolute Git executable and one analyzed repository.
    pub fn from_trusted_executable(git: PathBuf, repository: PathBuf) -> Option<Self> {
        (git.is_absolute() && repository.exists()).then_some(Self { git, repository })
    }

    fn git(&self, arguments: &[&std::ffi::OsStr]) -> Result<(), WorkspaceError> {
        let status = Command::new(&self.git)
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|_| WorkspaceError::SnapshotUnavailable)?;
        if status.success() {
            Ok(())
        } else {
            Err(WorkspaceError::SnapshotUnavailable)
        }
    }
}

impl SnapshotWorkspace for GitSnapshotWorkspace {
    fn materialize(&self, inputs: &ControlInputs<'_>) -> Result<PreparedWorkspace, WorkspaceError> {
        // The analyzed commit is host-resolved provenance, never repository
        // content, but it is still shape-checked before reaching a command.
        if !is_commit_hash(inputs.analyzed_commit()) {
            return Err(WorkspaceError::SnapshotUnavailable);
        }
        let root = tempfile::Builder::new()
            .prefix("assay-agent-workspace-")
            .tempdir()
            .map_err(|_| WorkspaceError::SnapshotUnavailable)?
            .keep();
        let snapshot = root.join("snapshot");
        let control = root.join("control");
        let prepared = (|| {
            self.git(&[
                "clone".as_ref(),
                "--quiet".as_ref(),
                "--no-hardlinks".as_ref(),
                "--".as_ref(),
                self.repository.as_os_str(),
                snapshot.as_os_str(),
            ])?;
            self.git(&[
                "-C".as_ref(),
                snapshot.as_os_str(),
                "-c".as_ref(),
                "advice.detachedHead=false".as_ref(),
                "checkout".as_ref(),
                "--quiet".as_ref(),
                "--detach".as_ref(),
                "--force".as_ref(),
                inputs.analyzed_commit().as_ref(),
            ])?;
            // The exact tree of the analyzed revision, without history.
            remove_tree(&snapshot.join(".git")).map_err(|_| WorkspaceError::SnapshotUnavailable)?;
            fs::create_dir_all(&control).map_err(|_| WorkspaceError::ControlWriteFailed)?;
            let write = |name: &str, contents: &[u8]| {
                fs::write(control.join(name), contents)
                    .map_err(|_| WorkspaceError::ControlWriteFailed)
            };
            let instructions = format!(
                "{}\n\n{}\n",
                inputs.instructions(),
                inputs.system_instructions()
            );
            write("instructions.txt", instructions.as_bytes())?;
            write("request.json", inputs.canonical_payload().as_bytes())?;
            let evidence = serde_json::to_vec_pretty(&inputs.evidence_ids())
                .map_err(|_| WorkspaceError::ControlWriteFailed)?;
            write("evidence-ids.json", &evidence)?;
            Ok(PreparedWorkspace::new(
                snapshot.clone(),
                control.clone(),
                control.join("judgment.json"),
            ))
        })();
        if prepared.is_err() {
            let _ = remove_tree(&root);
        }
        prepared
    }

    fn dispose(&self, workspace: PreparedWorkspace) {
        // Both directories share one temporary root; cleanup is best-effort.
        if let Some(root) = workspace.snapshot_dir().parent() {
            let _ = remove_tree(root);
        }
    }
}

pub(crate) fn is_commit_hash(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

// Removes a tree even when Git left read-only objects behind (Windows).
fn remove_tree(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    if fs::remove_dir_all(path).is_ok() {
        return Ok(());
    }
    clear_readonly(path)?;
    fs::remove_dir_all(path)
}

fn clear_readonly(path: &Path) -> std::io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    let mut permissions = metadata.permissions();
    if permissions.readonly() {
        #[allow(clippy::permissions_set_readonly_false)]
        permissions.set_readonly(false);
        fs::set_permissions(path, permissions)?;
    }
    if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            clear_readonly(&entry?.path())?;
        }
    }
    Ok(())
}
