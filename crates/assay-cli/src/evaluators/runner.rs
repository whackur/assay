use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use assay_ai_evaluator::{AgentIdentity, AgentRun, AgentRunError, AgentRunner, PreparedWorkspace};

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
