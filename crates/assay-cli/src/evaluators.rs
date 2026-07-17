//! Deployment-layer evaluator wiring: the static evaluator registry and the
//! concrete implementations of the `assay-ai-evaluator` ports (ADR 0012).
//!
//! The core evaluation crate performs no I/O; this module supplies the
//! environment-variable [`SecretStore`], the Git-backed [`SnapshotWorkspace`],
//! and the bounded agent-CLI [`AgentRunner`]. Trusted executables follow the
//! ADR 0002 pattern: an operator-set environment variable naming one absolute
//! path, never a `PATH` search and never repository content. Agentic
//! providers hold no Assay-managed secret; the agent's own credential store
//! is used in place through its official login.

use std::{
    ffi::OsString,
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use assay_ai_evaluator::{
    AgentIdentity, AgentRun, AgentRunError, AgentRunner, ControlInputs, PreparedWorkspace,
    ProviderSecret, SecretError, SecretName, SecretStore, SnapshotWorkspace, WorkspaceError,
};

/// Environment variable naming one trusted, absolute Codex CLI executable.
///
/// This is trusted deployment or startup configuration (ADR 0002 rule 1);
/// there is no default install location and no `PATH` search, so the agentic
/// provider is unavailable until an operator sets it explicitly.
pub const CODEX_CLI_EXECUTABLE_ENV: &str = "ASSAY_CODEX_CLI_EXECUTABLE";

/// The provider family an evaluator ID belongs to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvaluatorFamily {
    /// Deterministic evidence-only analysis; performs no AI evaluation.
    Deterministic,
    /// API-key HTTP providers receiving only the bounded evidence bundle.
    ApiKey,
    /// Agentic CLI providers exploring a whole worktree snapshot.
    Agentic,
}

impl EvaluatorFamily {
    /// Returns the stable machine-readable family code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Deterministic => "deterministic",
            Self::ApiKey => "api_key",
            Self::Agentic => "agentic",
        }
    }
}

/// One entry of the static evaluator registry selectable via `--evaluator`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EvaluatorDescriptor {
    id: &'static str,
    family: EvaluatorFamily,
    implemented: bool,
}

impl EvaluatorDescriptor {
    /// Returns the stable evaluator identifier.
    pub const fn id(&self) -> &'static str {
        self.id
    }

    /// Returns the provider family this evaluator belongs to.
    pub const fn family(&self) -> EvaluatorFamily {
        self.family
    }

    /// Reports whether this binary can actually run the evaluator end to end.
    ///
    /// The capabilities report derives from this flag and must stay honest:
    /// adapter code existing in the workspace is not enough; the evaluator is
    /// implemented only when an `assay project analyze` invocation can
    /// produce its validated evaluation.
    pub const fn is_implemented(&self) -> bool {
        self.implemented
    }
}

/// The static registry mapping stable evaluator IDs to a family (ADR 0012).
///
/// `deterministic` is the default and performs no AI evaluation. The AI
/// evaluator IDs are registered so automation can detect exactly which
/// providers this binary supports, but both stay `not_implemented`: the local
/// slice has no consent-granting surface, no live HTTP transport, and no
/// evaluation section in the analysis output, so no AI evaluation can
/// actually run end to end yet.
pub const EVALUATOR_REGISTRY: &[EvaluatorDescriptor] = &[
    EvaluatorDescriptor {
        id: "deterministic",
        family: EvaluatorFamily::Deterministic,
        implemented: true,
    },
    EvaluatorDescriptor {
        id: "openai-api-1",
        family: EvaluatorFamily::ApiKey,
        implemented: false,
    },
    EvaluatorDescriptor {
        id: "codex-cli-1",
        family: EvaluatorFamily::Agentic,
        implemented: false,
    },
];

/// Environment-variable secret store: the first concrete [`SecretStore`].
///
/// The [`SecretName`] is the environment variable *name*; the value is read
/// only here and never appears in arguments, logs, results, or records.
#[derive(Clone, Copy, Debug, Default)]
pub struct EnvSecretStore;

impl EnvSecretStore {
    fn from_value(value: Option<OsString>) -> Result<ProviderSecret, SecretError> {
        match value {
            None => Err(SecretError::NotFound),
            Some(value) if value.is_empty() => Err(SecretError::NotFound),
            Some(value) => match value.into_string() {
                Ok(value) => Ok(ProviderSecret::new(value)),
                Err(_) => Err(SecretError::Unavailable),
            },
        }
    }
}

impl SecretStore for EnvSecretStore {
    fn load(&self, name: &SecretName) -> Result<ProviderSecret, SecretError> {
        Self::from_value(std::env::var_os(name.as_str()))
    }
}

/// Resolves the trusted Codex CLI executable from the operator environment.
///
/// Only an absolute path is accepted; a relative value is untrusted and
/// ignored, and no `PATH` search ever happens (ADR 0002 rule 1).
pub fn trusted_codex_cli() -> Option<PathBuf> {
    resolve_trusted_agent(std::env::var_os(CODEX_CLI_EXECUTABLE_ENV))
}

// Pure resolution split out so the absolute-path contract is testable
// without mutating the process environment.
fn resolve_trusted_agent(value: Option<OsString>) -> Option<PathBuf> {
    let value = value?;
    if value.is_empty() {
        return None;
    }
    let path = PathBuf::from(value);
    path.is_absolute().then_some(path)
}

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

fn is_commit_hash(value: &str) -> bool {
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

/// Bounded Codex CLI [`AgentRunner`]: spawns one `codex exec` subprocess with
/// the snapshot as its constrained working directory, a read-only sandbox
/// request, a bounded wall-clock runtime, and a bounded judgment size. A
/// limit produces an explicit failure, never a fabricated result. The runner
/// holds no Assay-managed secret: the agent authenticates through its own
/// official login, and Assay never reads, copies, or transmits that store.
#[derive(Debug)]
pub struct CodexCliRunner {
    executable: PathBuf,
    timeout: Duration,
    max_output_bytes: usize,
}

impl CodexCliRunner {
    /// Binds a trusted absolute agent executable with explicit bounds.
    /// A non-absolute executable is untrusted and rejected.
    pub fn from_trusted_executable(
        executable: PathBuf,
        timeout: Duration,
        max_output_bytes: usize,
    ) -> Result<Self, AgentRunError> {
        if !executable.is_absolute() || timeout.is_zero() || max_output_bytes == 0 {
            return Err(AgentRunError::ProbeFailed);
        }
        Ok(Self {
            executable,
            timeout,
            max_output_bytes,
        })
    }

    fn wait_bounded(&self, mut child: std::process::Child) -> Result<bool, AgentRunError> {
        let deadline = Instant::now() + self.timeout;
        loop {
            match child.try_wait() {
                Ok(Some(status)) => return Ok(status.success()),
                Ok(None) if Instant::now() >= deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(AgentRunError::Timeout);
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(50)),
                Err(_) => return Err(AgentRunError::Failure),
            }
        }
    }

    fn read_bounded(&self, path: &Path) -> Result<Vec<u8>, AgentRunError> {
        let file = fs::File::open(path).map_err(|_| AgentRunError::Failure)?;
        let mut judgment = Vec::new();
        let read = file
            .take(self.max_output_bytes as u64 + 1)
            .read_to_end(&mut judgment)
            .map_err(|_| AgentRunError::Failure)?;
        if read > self.max_output_bytes {
            return Err(AgentRunError::OutputTooLarge);
        }
        Ok(judgment)
    }
}

impl AgentRunner for CodexCliRunner {
    fn probe(&self) -> Result<AgentIdentity, AgentRunError> {
        let child = Command::new(&self.executable)
            .arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| AgentRunError::ProbeFailed)?;
        let output = child
            .wait_with_output()
            .map_err(|_| AgentRunError::ProbeFailed)?;
        let version = String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()
            .unwrap_or_default()
            .trim()
            .to_owned();
        if !output.status.success() || version.is_empty() {
            return Err(AgentRunError::ProbeFailed);
        }
        Ok(AgentIdentity::new("codex".to_owned(), version))
    }

    fn run(&self, workspace: &PreparedWorkspace) -> Result<AgentRun, AgentRunError> {
        let prompt = format!(
            "Follow the instructions in {} exactly. The canonical request payload is {} and the only citable evidence IDs are listed in {}.",
            workspace.control_dir().join("instructions.txt").display(),
            workspace.control_dir().join("request.json").display(),
            workspace.control_dir().join("evidence-ids.json").display(),
        );
        let child = Command::new(&self.executable)
            .arg("exec")
            .arg("--sandbox")
            .arg("read-only")
            .arg("--skip-git-repo-check")
            .arg("--output-last-message")
            .arg(workspace.output_path())
            .arg(prompt)
            .current_dir(workspace.snapshot_dir())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| AgentRunError::Failure)?;
        let run_id = run_identifier(child.id());
        if !self.wait_bounded(child)? {
            return Err(AgentRunError::Failure);
        }
        let judgment = self.read_bounded(workspace.output_path())?;
        Ok(AgentRun::new(judgment, run_id))
    }
}

// One non-deterministic identifier per subprocess run.
fn run_identifier(process_id: u32) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    format!("run-{nanos:x}-{process_id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_ids_are_stable_and_honest() {
        let ids = EVALUATOR_REGISTRY
            .iter()
            .map(EvaluatorDescriptor::id)
            .collect::<Vec<_>>();
        assert_eq!(ids, ["deterministic", "openai-api-1", "codex-cli-1"]);
        // No AI evaluator may claim implemented until it can actually run.
        assert!(
            EVALUATOR_REGISTRY
                .iter()
                .filter(|descriptor| descriptor.family() != EvaluatorFamily::Deterministic)
                .all(|descriptor| !descriptor.is_implemented())
        );
    }

    #[test]
    fn env_secret_store_maps_values_without_exposing_them() {
        assert!(matches!(
            EnvSecretStore::from_value(None),
            Err(SecretError::NotFound)
        ));
        assert!(matches!(
            EnvSecretStore::from_value(Some(OsString::new())),
            Err(SecretError::NotFound)
        ));
        let secret = EnvSecretStore::from_value(Some(OsString::from("sk-test-value"))).unwrap();
        assert!(!format!("{secret:?}").contains("sk-test-value"));
    }

    #[test]
    fn agent_executable_resolution_requires_an_absolute_operator_path() {
        assert_eq!(resolve_trusted_agent(None), None);
        assert_eq!(resolve_trusted_agent(Some(OsString::new())), None);
        // A bare name would trigger a PATH search; it is untrusted.
        assert_eq!(resolve_trusted_agent(Some(OsString::from("codex"))), None);
        let absolute = std::env::current_exe().expect("test binary path");
        assert_eq!(
            resolve_trusted_agent(Some(absolute.clone().into_os_string())),
            Some(absolute)
        );
    }

    #[test]
    fn runner_rejects_untrusted_executables_and_empty_bounds() {
        assert!(
            CodexCliRunner::from_trusted_executable(
                PathBuf::from("codex"),
                Duration::from_secs(60),
                64 * 1024,
            )
            .is_err()
        );
        let absolute = std::env::current_exe().expect("test binary path");
        assert!(
            CodexCliRunner::from_trusted_executable(absolute.clone(), Duration::ZERO, 1).is_err()
        );
        assert!(
            CodexCliRunner::from_trusted_executable(absolute, Duration::from_secs(60), 64 * 1024)
                .is_ok()
        );
    }

    #[test]
    fn commit_hash_shape_is_enforced_before_any_command() {
        assert!(is_commit_hash(&"a1".repeat(20)));
        assert!(is_commit_hash(&"b2".repeat(32)));
        assert!(!is_commit_hash("HEAD"));
        assert!(!is_commit_hash("--upload-pack=echo"));
        assert!(!is_commit_hash(&"g".repeat(40)));
    }
}
