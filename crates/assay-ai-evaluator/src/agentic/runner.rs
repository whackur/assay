use super::workspace::PreparedWorkspace;

/// The probed identity of one trusted agent CLI installation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentIdentity {
    cli: String,
    version: String,
}

impl AgentIdentity {
    /// Records the agent CLI name and probed version.
    pub const fn new(cli: String, version: String) -> Self {
        Self { cli, version }
    }

    /// Returns the agent CLI identity, for example `codex`.
    pub fn cli(&self) -> &str {
        &self.cli
    }

    /// Returns the probed agent CLI version.
    pub fn version(&self) -> &str {
        &self.version
    }
}

/// One completed agent run: untrusted judgment bytes plus a run identifier.
pub struct AgentRun {
    judgment: Vec<u8>,
    run_id: String,
}

impl AgentRun {
    /// Records the collected output of one bounded agent subprocess.
    pub const fn new(judgment: Vec<u8>, run_id: String) -> Self {
        Self { judgment, run_id }
    }

    /// Returns the untrusted judgment bytes for the shared validator.
    pub fn judgment(&self) -> &[u8] {
        &self.judgment
    }

    /// Returns the non-deterministic identifier of this single run.
    pub fn run_id(&self) -> &str {
        &self.run_id
    }
}

impl std::fmt::Debug for AgentRun {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AgentRun")
            .field("judgment", &"<untrusted-provider-output>")
            .field("run_id", &self.run_id)
            .finish()
    }
}

/// Redacted agent-run failure. A limit always produces an explicit failure
/// rather than a fabricated result, and no variant retains process output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentRunError {
    /// No compatible, authenticated agent CLI installation was found.
    ProbeFailed,
    /// The bounded wall-clock runtime elapsed before a judgment was written.
    Timeout,
    /// The agent wrote more output than the configured bound allows.
    OutputTooLarge,
    /// The agent attempted to escape its read-only or write constraints.
    SandboxViolation,
    /// The subprocess failed or produced no judgment document.
    Failure,
}

/// Agent process seam. The concrete runner spawns one bounded subprocess
/// with the snapshot as its constrained working directory; it lives in the
/// deployment layer and resolves its executable per the ADR 0002 pattern.
pub trait AgentRunner {
    /// Probes the trusted agent CLI identity and version before any run.
    fn probe(&self) -> Result<AgentIdentity, AgentRunError>;

    /// Spawns one bounded agent subprocess constrained to the workspace and
    /// returns the untrusted judgment bytes it wrote to the output path.
    fn run(&self, workspace: &PreparedWorkspace) -> Result<AgentRun, AgentRunError>;
}
