use std::time::Duration;

use crate::{EvaluationErrorKind, ValidatedJudgmentSet};

/// Deterministic sampling settings recorded as scoring-independent provenance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SamplingConfig {
    pub temperature: f32,
    pub top_p: f32,
    pub max_output_tokens: u32,
    pub seed: Option<u64>,
}

/// Token usage reported by the provider. Non-deterministic, excluded from scores.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Judgment text and telemetry a profile extracted from a response body.
#[derive(Debug)]
pub struct ProviderReply {
    /// Untrusted judgment document text for the one shared validator.
    pub judgment: String,
    pub usage: Option<Usage>,
}

/// Non-deterministic call telemetry isolated from deterministic judgment inputs.
#[derive(Clone, Copy, Debug)]
pub struct ProviderTelemetry {
    pub(crate) http_status: u16,
    pub(crate) latency: Duration,
    pub(crate) usage: Option<Usage>,
    pub(crate) retry_after: Option<Duration>,
}

impl ProviderTelemetry {
    pub(crate) const fn from_response(
        http_status: u16,
        latency: Duration,
        retry_after: Option<Duration>,
        usage: Option<Usage>,
    ) -> Self {
        Self {
            http_status,
            latency,
            usage,
            retry_after,
        }
    }

    pub const fn http_status(&self) -> u16 {
        self.http_status
    }

    pub const fn latency(&self) -> Duration {
        self.latency
    }

    pub const fn usage(&self) -> Option<Usage> {
        self.usage
    }

    pub const fn retry_after(&self) -> Option<Duration> {
        self.retry_after
    }
}

/// Deterministic provenance recorded for every evaluation snapshot.
#[derive(Clone, Debug)]
pub struct SnapshotProvenance {
    pub(crate) provider_id: &'static str,
    pub(crate) model: String,
    pub(crate) prompt_version: &'static str,
    pub(crate) rubric_version: &'static str,
    pub(crate) evaluation_version: &'static str,
    pub(crate) sampling: SamplingConfig,
    pub(crate) evidence_bundle_hash: String,
}

impl SnapshotProvenance {
    pub const fn provider_id(&self) -> &'static str {
        self.provider_id
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub const fn prompt_version(&self) -> &'static str {
        self.prompt_version
    }

    pub const fn rubric_version(&self) -> &'static str {
        self.rubric_version
    }

    pub const fn evaluation_version(&self) -> &'static str {
        self.evaluation_version
    }

    pub const fn sampling(&self) -> SamplingConfig {
        self.sampling
    }

    /// Exact evidence-bundle hash presented to the provider.
    pub fn evidence_bundle_hash(&self) -> &str {
        &self.evidence_bundle_hash
    }
}

/// Explicit validation status: a validated judgment set or a named failure.
#[derive(Clone, Debug)]
pub enum SnapshotOutcome {
    Validated(ValidatedJudgmentSet),
    Failed(EvaluationErrorKind),
}

impl SnapshotOutcome {
    /// Stable status code that never disguises a failure as success.
    pub const fn status_code(&self) -> &'static str {
        match self {
            Self::Validated(_) => "validated",
            Self::Failed(kind) => kind.code(),
        }
    }
}

/// Honest, self-describing record of one provider evaluation attempt.
#[derive(Clone, Debug)]
pub struct EvaluationSnapshot {
    pub(crate) provenance: SnapshotProvenance,
    pub(crate) outcome: SnapshotOutcome,
    pub(crate) telemetry: Option<ProviderTelemetry>,
}

impl EvaluationSnapshot {
    pub const fn provenance(&self) -> &SnapshotProvenance {
        &self.provenance
    }

    pub const fn outcome(&self) -> &SnapshotOutcome {
        &self.outcome
    }

    /// Absent when no call completed.
    pub const fn telemetry(&self) -> Option<&ProviderTelemetry> {
        self.telemetry.as_ref()
    }

    /// Validated judgment set only on the success path.
    pub const fn validated(&self) -> Option<&ValidatedJudgmentSet> {
        match &self.outcome {
            SnapshotOutcome::Validated(set) => Some(set),
            SnapshotOutcome::Failed(_) => None,
        }
    }
}
