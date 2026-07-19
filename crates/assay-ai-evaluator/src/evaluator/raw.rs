use serde::Deserialize;

use crate::{EvidenceScope, ExternalTransmission};

use super::types::{Applicability, EvaluationStatus};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawPrivacy {
    pub(crate) evidence_scope: EvidenceScope,
    pub(crate) external_transmission: ExternalTransmission,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawJudgment {
    pub(crate) criterion_id: String,
    pub(crate) applicability: Applicability,
    pub(crate) rating: Option<i64>,
    pub(crate) rating_scale: i64,
    pub(crate) confidence: f64,
    pub(crate) evidence_ids: Vec<String>,
    pub(crate) rationale: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawJudgmentSet {
    pub(crate) schema_version: String,
    pub(crate) evaluation_version: String,
    pub(crate) rubric_version: String,
    pub(crate) status: EvaluationStatus,
    pub(crate) evidence_bundle_hash: String,
    pub(crate) privacy: RawPrivacy,
    pub(crate) judgments: Vec<RawJudgment>,
}
