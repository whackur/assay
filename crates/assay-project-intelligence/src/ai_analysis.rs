use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

pub const PROJECT_AI_ANALYSIS_CONTRACT: &str = "project-ai-analysis";
pub const PROJECT_AI_ANALYSIS_SCHEMA_VERSION: &str = "1.0.0";

#[derive(Clone, Serialize)]
pub struct ProjectAiAnalysisEnvelope {
    contract: &'static str,
    schema_version: &'static str,
    data: ProjectAiAnalysis,
}

impl ProjectAiAnalysisEnvelope {
    pub fn from_stored(input: StoredProjectAiAnalysis) -> Option<Self> {
        let document: JudgmentDocument = serde_json::from_value(input.judgment).ok()?;
        if document.schema_version != "1.0.0"
            || !matches!(document.status.as_str(), "complete" | "partial")
            || document.privacy.evidence_scope != "public_only"
            || !matches!(
                (
                    document.privacy.evidence_scope.as_str(),
                    document.privacy.external_transmission.as_str()
                ),
                ("public_only", "not_used" | "public_only")
            )
            || document.evaluation_version != input.evaluation_version
            || document.rubric_version != input.rubric_version
            || document.evidence_bundle_hash != input.evidence_bundle_hash
            || document.judgments.is_empty()
            || document.judgments.len() > 64
        {
            return None;
        }
        if document
            .judgments
            .iter()
            .any(|judgment| !valid_judgment(judgment))
        {
            return None;
        }
        Some(Self {
            contract: PROJECT_AI_ANALYSIS_CONTRACT,
            schema_version: PROJECT_AI_ANALYSIS_SCHEMA_VERSION,
            data: ProjectAiAnalysis {
                project: Project {
                    owner: input.owner,
                    repository: input.repository,
                },
                revision: Revision {
                    commit_sha: input.commit_sha,
                    default_branch: input.default_branch,
                },
                evaluation: Evaluation {
                    evaluation_version: input.evaluation_version,
                    rubric_version: input.rubric_version,
                    evidence_bundle_hash: input.evidence_bundle_hash,
                },
                interpretation: Interpretation {
                    kind: "ai_interpretation",
                    not_a_project_score: true,
                },
                judgments: document.judgments,
                limitations: vec![
                    "AI interpretation of cited public evidence only; it is not a project score."
                        .to_owned(),
                    "Missing or insufficient evidence is not treated as a zero.".to_owned(),
                ],
            },
        })
    }
}

fn valid_judgment(judgment: &Judgment) -> bool {
    if judgment.evidence_ids.len() > 32
        || judgment.rationale.is_empty()
        || judgment.rationale.len() > 1_000
        || judgment.rating_scale != 4
        || judgment.rating.is_some_and(|rating| rating > 4)
        || !(0.0..=1.0).contains(&judgment.confidence)
        || judgment.criterion_id.len() < 3
        || judgment.criterion_id.len() > 100
        || !matches!(
            judgment.applicability.as_str(),
            "applicable" | "partially_applicable" | "not_applicable"
        )
        || judgment
            .evidence_ids
            .iter()
            .any(|id| !valid_evidence_id(id))
    {
        return false;
    }
    if judgment.evidence_ids.len() != judgment.evidence_ids.iter().collect::<HashSet<_>>().len() {
        return false;
    }
    if judgment.applicability == "not_applicable" {
        judgment.rating.is_none() && judgment.evidence_ids.is_empty()
    } else {
        judgment.rating.is_some() && !judgment.evidence_ids.is_empty()
    }
}

fn valid_evidence_id(id: &str) -> bool {
    let mut parts = id.split(':');
    matches!(parts.next(), Some("evidence"))
        && id.matches(':').count() >= 2
        && parts.all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || ".-_".contains(c))
        })
}

#[derive(Clone)]
pub struct StoredProjectAiAnalysis {
    pub owner: String,
    pub repository: String,
    pub commit_sha: String,
    pub default_branch: String,
    pub evaluation_version: String,
    pub rubric_version: String,
    pub evidence_bundle_hash: String,
    pub judgment: Value,
}

#[derive(Clone, Serialize)]
struct ProjectAiAnalysis {
    project: Project,
    revision: Revision,
    evaluation: Evaluation,
    interpretation: Interpretation,
    judgments: Vec<Judgment>,
    limitations: Vec<String>,
}
#[derive(Clone, Serialize)]
struct Project {
    owner: String,
    repository: String,
}
#[derive(Clone, Serialize)]
struct Revision {
    commit_sha: String,
    default_branch: String,
}
#[derive(Clone, Serialize)]
struct Evaluation {
    evaluation_version: String,
    rubric_version: String,
    evidence_bundle_hash: String,
}
#[derive(Clone, Serialize)]
struct Interpretation {
    kind: &'static str,
    not_a_project_score: bool,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct JudgmentDocument {
    schema_version: String,
    evaluation_version: String,
    rubric_version: String,
    status: String,
    evidence_bundle_hash: String,
    privacy: Privacy,
    judgments: Vec<Judgment>,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Privacy {
    evidence_scope: String,
    external_transmission: String,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Judgment {
    criterion_id: String,
    applicability: String,
    rating: Option<u8>,
    rating_scale: u8,
    confidence: f64,
    evidence_ids: Vec<String>,
    rationale: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn stored(judgment: Value) -> StoredProjectAiAnalysis {
        StoredProjectAiAnalysis {
            owner: "assay".into(),
            repository: "example".into(),
            commit_sha: "0123456789abcdef0123456789abcdef01234567".into(),
            default_branch: "main".into(),
            evaluation_version: "hosted-evaluation-1".into(),
            rubric_version: "project-rubric-1".into(),
            evidence_bundle_hash:
                "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
            judgment,
        }
    }

    #[test]
    fn projects_only_public_bound_validated_judgments() {
        let document = json!({
            "schema_version": "1.0.0", "evaluation_version": "hosted-evaluation-1",
            "rubric_version": "project-rubric-1", "status": "complete",
            "evidence_bundle_hash": "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "privacy": {"evidence_scope": "public_only", "external_transmission": "not_used"},
            "judgments": [{"criterion_id": "quality.documentation", "applicability": "applicable", "rating": 3, "rating_scale": 4, "confidence": 0.8, "evidence_ids": ["evidence:github:readme"], "rationale": "Clear public documentation."}]
        });
        let output = ProjectAiAnalysisEnvelope::from_stored(stored(document.clone())).unwrap();
        let value = serde_json::to_value(output).unwrap();
        assert_eq!(value["data"]["interpretation"]["not_a_project_score"], true);
        assert!(value["data"]["judgments"][0].get("rationale").is_some());

        let mut public_transmission = document;
        public_transmission["privacy"]["external_transmission"] = json!("public_only");
        assert!(ProjectAiAnalysisEnvelope::from_stored(stored(public_transmission)).is_some());
    }

    #[test]
    fn rejects_private_or_mismatched_judgments() {
        let mut document = json!({
            "schema_version": "1.0.0", "evaluation_version": "hosted-evaluation-1",
            "rubric_version": "project-rubric-1", "status": "complete",
            "evidence_bundle_hash": "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "privacy": {"evidence_scope": "private_local", "external_transmission": "not_used"}, "judgments": []
        });
        assert!(ProjectAiAnalysisEnvelope::from_stored(stored(document.clone())).is_none());
        document["privacy"]["evidence_scope"] = json!("public_only");
        document["privacy"]["external_transmission"] = json!("not_used");
        document["evaluation_version"] = json!("legacy");
        assert!(ProjectAiAnalysisEnvelope::from_stored(stored(document)).is_none());
    }

    #[test]
    fn rejects_schema_invalid_judgment_fields() {
        let base = json!({
            "schema_version": "1.0.0", "evaluation_version": "hosted-evaluation-1",
            "rubric_version": "project-rubric-1", "status": "complete",
            "evidence_bundle_hash": "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "privacy": {"evidence_scope": "public_only", "external_transmission": "public_only"},
            "judgments": [{"criterion_id": "quality.documentation", "applicability": "applicable", "rating": 3, "rating_scale": 4, "confidence": 0.8, "evidence_ids": ["evidence:github:readme", "evidence:github:readme"], "rationale": "Supported."}]
        });
        assert!(ProjectAiAnalysisEnvelope::from_stored(stored(base.clone())).is_none());
        let mut empty_rationale = base;
        empty_rationale["judgments"][0]["evidence_ids"] = json!(["evidence:github:readme"]);
        empty_rationale["judgments"][0]["rationale"] = json!("");
        assert!(ProjectAiAnalysisEnvelope::from_stored(stored(empty_rationale)).is_none());
    }
}
