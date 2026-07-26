mod runner;
mod secret;
mod workspace;

#[cfg(test)]
mod tests;

pub use runner::CodexCliRunner;
pub use secret::EnvSecretStore;
pub use workspace::GitSnapshotWorkspace;

use std::{ffi::OsString, path::PathBuf};

/// Environment variable naming one trusted, absolute Codex CLI executable. Trusted operator config (ADR 0002 rule 1); no default, no `PATH` search.
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

    /// Reports whether this binary can actually run the evaluator end to end. Adapter code existing in the workspace is not enough.
    pub const fn is_implemented(&self) -> bool {
        self.implemented
    }
}

/// Static registry mapping stable evaluator IDs to a family (ADR 0012). `deterministic` is default; AI IDs stay `not_implemented` pending a consent-granting surface.
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

/// Resolves the trusted Codex CLI executable from the operator environment. Only absolute paths accepted; no `PATH` search (ADR 0002 rule 1).
pub fn trusted_codex_cli() -> Option<PathBuf> {
    resolve_trusted_agent(std::env::var_os(CODEX_CLI_EXECUTABLE_ENV))
}

// Pure split so the absolute-path contract is testable without mutating process env.
pub(crate) fn resolve_trusted_agent(value: Option<OsString>) -> Option<PathBuf> {
    let value = value?;
    if value.is_empty() {
        return None;
    }
    let path = PathBuf::from(value);
    path.is_absolute().then_some(path)
}
