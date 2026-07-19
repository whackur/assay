use serde::{Deserialize, Deserializer, Serialize, de};

use crate::error::DomainValueError;
use crate::evidence::EvidenceSource;
use crate::identifiers::{AnalysisVersion, RuleSetHash};
use crate::snapshot::SourceSnapshot;
use crate::status::{AnalysisStatus, EvidenceStatus};
use crate::warning::{Limitation, Warning};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AnalysisManifestData {
    source_snapshot: SourceSnapshot,
    analysis_version: AnalysisVersion,
    rule_set_hash: RuleSetHash,
    status: AnalysisStatus,
    evidence_sources: Vec<EvidenceSource>,
    warnings: Vec<Warning>,
    limitations: Vec<Limitation>,
}

/// Deterministic domain manifest for one immutable source snapshot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AnalysisManifest {
    source_snapshot: SourceSnapshot,
    analysis_version: AnalysisVersion,
    rule_set_hash: RuleSetHash,
    status: AnalysisStatus,
    evidence_sources: Vec<EvidenceSource>,
    warnings: Vec<Warning>,
    limitations: Vec<Limitation>,
}

impl AnalysisManifest {
    /// Creates a manifest and canonicalizes all identifier-keyed collections.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source_snapshot: SourceSnapshot,
        analysis_version: AnalysisVersion,
        rule_set_hash: RuleSetHash,
        status: AnalysisStatus,
        mut evidence_sources: Vec<EvidenceSource>,
        mut warnings: Vec<Warning>,
        mut limitations: Vec<Limitation>,
    ) -> Result<Self, DomainValueError> {
        if evidence_sources.is_empty() {
            return Err(DomainValueError::new(
                "analysis_manifest",
                "at least one explicit evidence source is required",
            ));
        }
        if status == AnalysisStatus::Complete
            && evidence_sources
                .iter()
                .any(|source| source.status() != EvidenceStatus::Complete)
        {
            return Err(DomainValueError::new(
                "analysis_manifest",
                "complete analysis requires every evidence source to be complete",
            ));
        }

        evidence_sources.sort_by(|left, right| left.id().cmp(right.id()));
        if evidence_sources
            .windows(2)
            .any(|pair| pair[0].id() == pair[1].id())
        {
            return Err(DomainValueError::new(
                "analysis_manifest",
                "evidence identifiers must be unique",
            ));
        }

        warnings.sort_by(|left, right| left.code().cmp(right.code()));
        if warnings
            .windows(2)
            .any(|pair| pair[0].code() == pair[1].code())
        {
            return Err(DomainValueError::new(
                "analysis_manifest",
                "warning codes must be unique",
            ));
        }

        limitations.sort_by(|left, right| left.code().cmp(right.code()));
        if limitations
            .windows(2)
            .any(|pair| pair[0].code() == pair[1].code())
        {
            return Err(DomainValueError::new(
                "analysis_manifest",
                "limitation codes must be unique",
            ));
        }

        Ok(Self {
            source_snapshot,
            analysis_version,
            rule_set_hash,
            status,
            evidence_sources,
            warnings,
            limitations,
        })
    }

    /// Returns the overall analysis state without changing evidence states.
    pub const fn status(&self) -> AnalysisStatus {
        self.status
    }

    /// Returns the immutable source snapshot.
    pub const fn source_snapshot(&self) -> &SourceSnapshot {
        &self.source_snapshot
    }

    /// Returns the analysis contract version.
    pub const fn analysis_version(&self) -> &AnalysisVersion {
        &self.analysis_version
    }

    /// Returns the hash of the complete effective rule set.
    pub const fn rule_set_hash(&self) -> &RuleSetHash {
        &self.rule_set_hash
    }

    /// Returns evidence sources in canonical evidence-ID order.
    pub fn evidence_sources(&self) -> &[EvidenceSource] {
        &self.evidence_sources
    }

    /// Returns warnings in canonical code order.
    pub fn warnings(&self) -> &[Warning] {
        &self.warnings
    }

    /// Returns limitations in canonical code order.
    pub fn limitations(&self) -> &[Limitation] {
        &self.limitations
    }
}

impl TryFrom<AnalysisManifestData> for AnalysisManifest {
    type Error = DomainValueError;

    fn try_from(value: AnalysisManifestData) -> Result<Self, Self::Error> {
        Self::new(
            value.source_snapshot,
            value.analysis_version,
            value.rule_set_hash,
            value.status,
            value.evidence_sources,
            value.warnings,
            value.limitations,
        )
    }
}

impl<'de> Deserialize<'de> for AnalysisManifest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_from(AnalysisManifestData::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}
