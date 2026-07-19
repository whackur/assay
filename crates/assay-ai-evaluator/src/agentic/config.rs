use crate::ProviderExecutionBoundary;

/// Deployment configuration for one agentic CLI provider.
#[derive(Clone, Debug)]
pub struct AgenticConfig {
    /// Stable adapter identifier recorded as provenance, e.g. `codex-cli-1`.
    pub provider_id: &'static str,
    /// The model identifier the agent CLI is configured to use.
    pub model: String,
    /// `External` whenever the agent's model endpoint is remote, even though
    /// the process runs locally; `Local` only for a fully local endpoint.
    pub execution_boundary: ProviderExecutionBoundary,
    /// The resolved commit of the analyzed revision the snapshot materializes.
    pub analyzed_commit: String,
}
