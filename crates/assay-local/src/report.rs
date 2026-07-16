//! Versioned local report envelope rendered by `assay serve`.
//!
//! It wraps the deterministic project-analysis payload with private-feature
//! section reports and privacy metadata. A local report is always
//! `private_local` and never catalog-eligible, so private source or its
//! derivatives cannot enter the public catalog or comparison corpus.

use serde::Serialize;
use serde_json::Value;

use crate::consent::{ConsentState, ExternalTransmission, PrivateFeature, SectionReport};

/// The versioned schema identifier for the local report contract.
pub const LOCAL_REPORT_SCHEMA_VERSION: &str = "1.0.0";

/// A failure to build a local report from an analysis payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalReportError {
    reason: &'static str,
}

impl LocalReportError {
    /// Returns a machine-stable reason code.
    pub const fn reason(self) -> &'static str {
        self.reason
    }
}

impl std::fmt::Display for LocalReportError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid local report: {}", self.reason)
    }
}

impl std::error::Error for LocalReportError {}

#[derive(Debug, Serialize)]
struct Sections {
    ai_evaluation: SectionReport,
    competitor_discovery: SectionReport,
}

#[derive(Debug, Serialize)]
struct Privacy {
    visibility: &'static str,
    source_content: &'static str,
    external_transmission: ExternalTransmission,
    catalog_eligible: bool,
}

/// A local dashboard report combining deterministic analysis with consent-gated
/// private-feature sections.
#[derive(Debug, Serialize)]
pub struct LocalReport {
    schema_version: &'static str,
    visibility: &'static str,
    repository: Value,
    generated_at: String,
    analysis: Value,
    sections: Sections,
    privacy: Privacy,
}

impl LocalReport {
    /// Builds a report from a project-analysis payload and a consent posture.
    ///
    /// The analysis source must be `local`; hosted sources are rejected because
    /// the local dashboard renders only local, non-catalog records.
    pub fn from_analysis(
        analysis: Value,
        consent: &ConsentState,
        generated_at: impl Into<String>,
    ) -> Result<Self, LocalReportError> {
        let source = analysis
            .get("manifest")
            .and_then(|manifest| manifest.get("source_snapshot"))
            .and_then(|snapshot| snapshot.get("source"))
            .ok_or(LocalReportError {
                reason: "analysis is missing a repository source",
            })?;
        if source.get("kind").and_then(Value::as_str) != Some("local") {
            return Err(LocalReportError {
                reason: "local report requires a local repository source",
            });
        }
        let repository = source.clone();
        Ok(Self {
            schema_version: LOCAL_REPORT_SCHEMA_VERSION,
            visibility: "private_local",
            repository,
            generated_at: generated_at.into(),
            analysis,
            sections: Sections {
                ai_evaluation: consent.section(PrivateFeature::AiEvaluation),
                competitor_discovery: consent.section(PrivateFeature::CompetitorDiscovery),
            },
            privacy: Privacy {
                visibility: "private_local",
                source_content: "not_retained",
                external_transmission: consent.external_transmission(),
                catalog_eligible: false,
            },
        })
    }

    /// Local reports are structurally excluded from the public catalog.
    pub const fn is_catalog_eligible(&self) -> bool {
        false
    }

    /// Returns the local repository identifier component, if present.
    pub fn repository_id(&self) -> Option<&str> {
        self.repository.get("repository_id").and_then(Value::as_str)
    }

    /// Serializes the report to a stable JSON value.
    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).expect("local report serializes to json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn local_analysis() -> Value {
        json!({
            "schema_version": "1.0.0",
            "manifest": {
                "source_snapshot": {
                    "source": { "kind": "local", "repository_id": "abc123" }
                }
            },
            "evidence": []
        })
    }

    #[test]
    fn builds_private_local_report_with_disabled_sections() {
        let report = LocalReport::from_analysis(
            local_analysis(),
            &ConsentState::default(),
            "2026-07-16T00:00:00Z",
        )
        .expect("build report");
        let value = report.to_value();
        assert_eq!(value["visibility"], "private_local");
        assert_eq!(value["privacy"]["catalog_eligible"], false);
        assert_eq!(
            value["privacy"]["external_transmission"],
            "consent_required"
        );
        assert_eq!(value["sections"]["ai_evaluation"]["state"], "disabled");
        assert_eq!(
            value["sections"]["ai_evaluation"]["reason"],
            "user_consent_required"
        );
        assert_eq!(report.repository_id(), Some("abc123"));
        assert!(!report.is_catalog_eligible());
    }

    #[test]
    fn rejects_hosted_source() {
        let analysis = json!({
            "manifest": { "source_snapshot": { "source": {
                "kind": "hosted", "provider": "github", "namespace": "o", "repository": "r"
            } } }
        });
        let error =
            LocalReport::from_analysis(analysis, &ConsentState::default(), "t").unwrap_err();
        assert_eq!(
            error.reason(),
            "local report requires a local repository source"
        );
    }
}
