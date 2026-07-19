use std::{collections::BTreeSet, str::FromStr};

use assay_domain::EvidenceId;

use crate::{
    AI_JUDGMENT_SCHEMA_VERSION, EvaluationError, EvaluationErrorKind, EvidenceBundle,
    QualitativeCriterion, QualitativeRubric,
    bundle::{TextPolicy, id_set, validate_untrusted_text},
};

use super::judgment::{ValidatedJudgment, ValidatedJudgmentSet, ValidatedPrivacy};
use super::provider::EvaluationProvider;
use super::raw::{RawJudgment, RawJudgmentSet};
use super::request::ProviderRequest;
use super::types::Applicability;

const MAX_PROVIDER_OUTPUT_BYTES: usize = 64 * 1024;

/// Validates untrusted provider output before any score compiler can consume it.
#[derive(Clone, Copy, Debug)]
pub struct Evaluator {
    rubric: QualitativeRubric,
}

impl Evaluator {
    /// Creates an evaluator for one immutable rubric version.
    pub const fn new(rubric: QualitativeRubric) -> Self {
        Self { rubric }
    }

    /// Calls a provider and returns only schema-shaped, cited, canonical judgments.
    pub fn evaluate<P: EvaluationProvider>(
        &self,
        provider: &P,
        bundle: &EvidenceBundle,
    ) -> Result<ValidatedJudgmentSet, EvaluationError> {
        super::boundary::enforce_transmission_boundary(
            provider.execution_boundary(),
            provider.transmission_surface(),
            bundle,
        )?;
        let request = ProviderRequest::new(self.rubric, bundle)?;
        let bytes = provider
            .evaluate(&request)
            .map_err(|_| EvaluationError::new(EvaluationErrorKind::ProviderFailure))?;
        self.validate_bytes(&bytes, bundle)
    }

    /// Returns the immutable rubric bound to this evaluator.
    pub(crate) const fn rubric(&self) -> QualitativeRubric {
        self.rubric
    }

    /// Validates untrusted provider bytes without assuming a provider transport.
    pub(crate) fn validate_bytes(
        &self,
        bytes: &[u8],
        bundle: &EvidenceBundle,
    ) -> Result<ValidatedJudgmentSet, EvaluationError> {
        if bytes.len() > MAX_PROVIDER_OUTPUT_BYTES {
            return Err(EvaluationError::new(EvaluationErrorKind::OutputTooLarge));
        }
        let raw: RawJudgmentSet = serde_json::from_slice(bytes).map_err(|error| {
            if error.is_syntax() || error.is_eof() {
                EvaluationError::new(EvaluationErrorKind::MalformedOutput)
            } else {
                EvaluationError::new(EvaluationErrorKind::SchemaInvalid)
            }
        })?;
        self.validate(raw, bundle)
    }

    fn validate(
        &self,
        raw: RawJudgmentSet,
        bundle: &EvidenceBundle,
    ) -> Result<ValidatedJudgmentSet, EvaluationError> {
        if raw.schema_version != AI_JUDGMENT_SCHEMA_VERSION {
            return Err(EvaluationError::new(EvaluationErrorKind::SchemaInvalid));
        }
        if raw.evaluation_version != self.rubric.evaluation_version() {
            return Err(EvaluationError::new(
                EvaluationErrorKind::EvaluationVersionMismatch,
            ));
        }
        if raw.rubric_version != self.rubric.version() {
            return Err(EvaluationError::new(
                EvaluationErrorKind::RubricVersionMismatch,
            ));
        }
        if raw.evidence_bundle_hash != bundle.content_hash() {
            if !is_sha256(&raw.evidence_bundle_hash) {
                return Err(EvaluationError::new(EvaluationErrorKind::SchemaInvalid));
            }
            return Err(EvaluationError::new(
                EvaluationErrorKind::EvidenceBundleMismatch,
            ));
        }
        if raw.privacy.evidence_scope != bundle.scope()
            || raw.privacy.external_transmission != bundle.transmission()
        {
            return Err(EvaluationError::new(EvaluationErrorKind::PrivacyMismatch));
        }
        if raw.status.is_usable() && raw.judgments.is_empty() {
            return Err(EvaluationError::new(EvaluationErrorKind::SchemaInvalid));
        }
        if !raw.status.is_usable() && !raw.judgments.is_empty() {
            return Err(EvaluationError::new(EvaluationErrorKind::SchemaInvalid));
        }
        let known_evidence = id_set(bundle);
        let mut criteria = BTreeSet::new();
        let mut judgments = Vec::with_capacity(raw.judgments.len());
        for judgment in raw.judgments {
            let criterion = self
                .rubric
                .criterion(&judgment.criterion_id)
                .ok_or_else(|| EvaluationError::new(EvaluationErrorKind::UnknownCriterion))?;
            if !criteria.insert(judgment.criterion_id.clone()) {
                return Err(EvaluationError::new(
                    EvaluationErrorKind::DuplicateCriterion,
                ));
            }
            let rating_scale = u8::try_from(judgment.rating_scale)
                .map_err(|_| EvaluationError::new(EvaluationErrorKind::InvalidRating))?;
            if rating_scale != criterion.rating_scale() {
                return Err(EvaluationError::new(EvaluationErrorKind::InvalidRating));
            }
            let rating = validate_rating(&judgment, criterion)?;
            if !judgment.confidence.is_finite() || !(0.0..=1.0).contains(&judgment.confidence) {
                return Err(EvaluationError::new(EvaluationErrorKind::InvalidConfidence));
            }
            if judgment.applicability != Applicability::NotApplicable
                && judgment.evidence_ids.is_empty()
            {
                return Err(EvaluationError::new(EvaluationErrorKind::MissingCitation));
            }
            let mut seen = BTreeSet::new();
            let mut evidence_ids = Vec::with_capacity(judgment.evidence_ids.len());
            for value in judgment.evidence_ids {
                if !seen.insert(value.clone()) {
                    return Err(EvaluationError::new(EvaluationErrorKind::DuplicateCitation));
                }
                let id = EvidenceId::from_str(&value)
                    .map_err(|_| EvaluationError::new(EvaluationErrorKind::SchemaInvalid))?;
                if !known_evidence.contains(value.as_str()) {
                    return Err(EvaluationError::new(
                        EvaluationErrorKind::UnknownEvidenceCitation,
                    ));
                }
                if !bundle.contains(&id) {
                    return Err(EvaluationError::new(
                        EvaluationErrorKind::UnknownEvidenceCitation,
                    ));
                }
                evidence_ids.push(id);
            }
            evidence_ids.sort();
            validate_untrusted_text(&judgment.rationale, TextPolicy::ProviderRationale)?;
            judgments.push(ValidatedJudgment {
                criterion_id: judgment.criterion_id,
                applicability: judgment.applicability,
                rating,
                rating_scale,
                confidence: judgment.confidence,
                evidence_ids,
                rationale: judgment.rationale,
            });
        }
        if raw.status == super::types::EvaluationStatus::Complete
            && criteria.len() != self.rubric.criteria().len()
        {
            return Err(EvaluationError::new(EvaluationErrorKind::MissingCriterion));
        }
        judgments.sort_by(|left, right| left.criterion_id.cmp(&right.criterion_id));
        Ok(ValidatedJudgmentSet {
            schema_version: AI_JUDGMENT_SCHEMA_VERSION.to_owned(),
            evaluation_version: self.rubric.evaluation_version().to_owned(),
            rubric_version: self.rubric.version().to_owned(),
            status: raw.status,
            evidence_bundle_hash: bundle.content_hash().to_owned(),
            privacy: ValidatedPrivacy {
                evidence_scope: bundle.scope(),
                external_transmission: bundle.transmission(),
            },
            judgments,
        })
    }
}

fn is_sha256(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    })
}

fn validate_rating(
    judgment: &RawJudgment,
    criterion: &QualitativeCriterion,
) -> Result<Option<u8>, EvaluationError> {
    match (judgment.applicability, judgment.rating) {
        (Applicability::NotApplicable, None) => Ok(None),
        (Applicability::NotApplicable, Some(_)) | (_, None) => {
            Err(EvaluationError::new(EvaluationErrorKind::InvalidRating))
        }
        (_, Some(value)) => {
            let rating = u8::try_from(value)
                .map_err(|_| EvaluationError::new(EvaluationErrorKind::InvalidRating))?;
            if rating > criterion.rating_scale() {
                return Err(EvaluationError::new(EvaluationErrorKind::InvalidRating));
            }
            Ok(Some(rating))
        }
    }
}
