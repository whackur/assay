mod runner;
mod secret;
mod workspace;

#[cfg(test)]
mod tests;

pub use runner::CodexCliRunner;
pub use secret::EnvSecretStore;
pub use workspace::GitSnapshotWorkspace;

use std::{ffi::OsString, path::PathBuf};

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

/// Resolves the trusted Codex CLI executable from the operator environment.
///
/// Only an absolute path is accepted; a relative value is untrusted and
/// ignored, and no `PATH` search ever happens (ADR 0002 rule 1).
pub fn trusted_codex_cli() -> Option<PathBuf> {
    resolve_trusted_agent(std::env::var_os(CODEX_CLI_EXECUTABLE_ENV))
}

// Pure resolution split out so the absolute-path contract is testable
// without mutating the process environment.
pub(crate) fn resolve_trusted_agent(value: Option<OsString>) -> Option<PathBuf> {
    let value = value?;
    if value.is_empty() {
        return None;
    }
    let path = PathBuf::from(value);
    path.is_absolute().then_some(path)
}
