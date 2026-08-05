use crate::api::SnapshotOutcome;

use super::runner::AgentIdentity;

/// Deterministic and per-run provenance recorded for every agentic snapshot.
/// Each `evaluate()` call is one run; results are never aggregated here.
/// Multi-run aggregation is deterministic post-processing on the
/// score-compiler side, outside the provider and outside this crate.
#[derive(Clone, Debug)]
pub struct AgenticProvenance {
    pub(crate) provider_id: &'static str,
    pub(crate) analyzed_commit: String,
    pub(crate) model: String,
    pub(crate) prompt_version: &'static str,
    pub(crate) rubric_version: &'static str,
    pub(crate) evaluation_version: &'static str,
    pub(crate) evidence_bundle_hash: String,
    pub(crate) agent: Option<AgentIdentity>,
    pub(crate) run_id: Option<String>,
}

impl AgenticProvenance {
    pub const fn provider_id(&self) -> &'static str {
        self.provider_id
    }

    /// Resolved commit of the analyzed revision.
    pub fn analyzed_commit(&self) -> &str {
        &self.analyzed_commit
    }

    /// Configured model identifier.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Shared prompt-envelope version.
    pub const fn prompt_version(&self) -> &'static str {
        self.prompt_version
    }

    /// Rubric version bound to the request.
    pub const fn rubric_version(&self) -> &'static str {
        self.rubric_version
    }

    /// Evaluation version bound to the request.
    pub const fn evaluation_version(&self) -> &'static str {
        self.evaluation_version
    }

    /// Exact evidence-bundle hash the judgment must bind to.
    pub fn evidence_bundle_hash(&self) -> &str {
        &self.evidence_bundle_hash
    }

    /// Probed agent CLI identity, absent before a probe succeeds.
    pub const fn agent(&self) -> Option<&AgentIdentity> {
        self.agent.as_ref()
    }

    /// Run identifier, present only once a subprocess completed.
    pub fn run_id(&self) -> Option<&str> {
        self.run_id.as_deref()
    }
}

/// Honest, self-describing record of one agentic evaluation attempt.
#[derive(Debug)]
pub struct AgenticSnapshot {
    pub(crate) provenance: AgenticProvenance,
    pub(crate) outcome: SnapshotOutcome,
}

impl AgenticSnapshot {
    /// Provenance recorded regardless of outcome.
    pub const fn provenance(&self) -> &AgenticProvenance {
        &self.provenance
    }

    /// Explicit validation outcome.
    pub const fn outcome(&self) -> &SnapshotOutcome {
        &self.outcome
    }
}
