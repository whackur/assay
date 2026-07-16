use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, de};

use crate::{ContentHash, DomainValueError, EvidenceId, EvidenceStatus};

const MAX_CRITERION_LENGTH: usize = 100;

/// Applicability of a rubric criterion or deterministic check to the classified
/// project. `NotApplicable` is an explicit status, never a zero score.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RubricApplicability {
    Applicable,
    PartiallyApplicable,
    NotApplicable,
}

fn validate_rubric_criterion_id(value: &str) -> Result<(), &'static str> {
    if value.is_empty() || value.len() > MAX_CRITERION_LENGTH {
        return Err("expected a non-empty dotted identifier of at most 100 characters");
    }
    let mut segments = value.split('.');
    let mut count = 0;
    for segment in &mut segments {
        count += 1;
        let bytes = segment.as_bytes();
        if bytes.is_empty() || !bytes[0].is_ascii_lowercase() {
            return Err("expected each segment to begin with a lowercase letter");
        }
        if !bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_')
        {
            return Err("expected lowercase snake_case segments");
        }
    }
    if count < 2 {
        return Err("expected at least two dot-separated segments");
    }
    Ok(())
}

/// A validated dotted rubric criterion identifier such as
/// `substance.claim_implementation_fit`.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RubricCriterionId(String);

impl RubricCriterionId {
    /// Returns the canonical dotted identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the leading dimension segment before the first dot.
    pub fn dimension_prefix(&self) -> &str {
        self.0.split('.').next().unwrap_or(&self.0)
    }
}

impl FromStr for RubricCriterionId {
    type Err = DomainValueError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        validate_rubric_criterion_id(value)
            .map_err(|reason| DomainValueError::new("rubric_criterion_id", reason))?;
        Ok(Self(value.to_owned()))
    }
}

impl TryFrom<String> for RubricCriterionId {
    type Error = DomainValueError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        validate_rubric_criterion_id(&value)
            .map_err(|reason| DomainValueError::new("rubric_criterion_id", reason))?;
        Ok(Self(value))
    }
}

impl Serialize for RubricCriterionId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for RubricCriterionId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_from(String::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

/// One validated qualitative rubric judgment consumed by the score compiler.
///
/// This is the provider-independent contract the deterministic compiler reads.
/// It carries bounded ratings and citations only; it can never carry a final
/// dimension or overall score, so a provider cannot emit or override a published
/// score through it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RubricJudgment {
    criterion_id: RubricCriterionId,
    applicability: RubricApplicability,
    rating: Option<u8>,
    rating_scale: u8,
    confidence: f64,
    evidence_ids: Vec<EvidenceId>,
}

impl RubricJudgment {
    /// Validates one bounded, cited rubric judgment.
    ///
    /// A `NotApplicable` criterion carries no rating and may cite no evidence.
    /// Every other applicability requires a rating within the inclusive scale
    /// and at least one citation.
    pub fn new(
        criterion_id: RubricCriterionId,
        applicability: RubricApplicability,
        rating: Option<u8>,
        rating_scale: u8,
        confidence: f64,
        mut evidence_ids: Vec<EvidenceId>,
    ) -> Result<Self, DomainValueError> {
        if rating_scale == 0 {
            return Err(DomainValueError::new(
                "rubric_judgment",
                "rating scale must be a positive inclusive bound",
            ));
        }
        match (applicability, rating) {
            (RubricApplicability::NotApplicable, Some(_)) => {
                return Err(DomainValueError::new(
                    "rubric_judgment",
                    "a not_applicable criterion must not carry a rating",
                ));
            }
            (RubricApplicability::NotApplicable, None) => {}
            (_, None) => {
                return Err(DomainValueError::new(
                    "rubric_judgment",
                    "an applicable criterion requires a rating",
                ));
            }
            (_, Some(value)) if value > rating_scale => {
                return Err(DomainValueError::new(
                    "rubric_judgment",
                    "a rating must not exceed the inclusive rating scale",
                ));
            }
            (_, Some(_)) => {}
        }
        if !confidence.is_finite() || !(0.0..=1.0).contains(&confidence) {
            return Err(DomainValueError::new(
                "rubric_judgment",
                "confidence must be a finite value in the closed unit interval",
            ));
        }
        if applicability != RubricApplicability::NotApplicable && evidence_ids.is_empty() {
            return Err(DomainValueError::new(
                "rubric_judgment",
                "an applicable criterion requires at least one cited evidence identifier",
            ));
        }
        evidence_ids.sort();
        if evidence_ids.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(DomainValueError::new(
                "rubric_judgment",
                "cited evidence identifiers must be unique",
            ));
        }
        Ok(Self {
            criterion_id,
            applicability,
            rating,
            rating_scale,
            confidence,
            evidence_ids,
        })
    }

    /// Returns the stable dotted criterion identifier.
    pub const fn criterion_id(&self) -> &RubricCriterionId {
        &self.criterion_id
    }

    /// Returns criterion applicability without inventing a zero score.
    pub const fn applicability(&self) -> RubricApplicability {
        self.applicability
    }

    /// Returns the bounded rating, absent only when not applicable.
    pub const fn rating(&self) -> Option<u8> {
        self.rating
    }

    /// Returns the inclusive rating upper bound.
    pub const fn rating_scale(&self) -> u8 {
        self.rating_scale
    }

    /// Returns validated provider confidence in the closed unit interval.
    pub const fn confidence(&self) -> f64 {
        self.confidence
    }

    /// Returns cited evidence identifiers in canonical order.
    pub fn evidence_ids(&self) -> &[EvidenceId] {
        &self.evidence_ids
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RubricJudgmentData {
    criterion_id: RubricCriterionId,
    applicability: RubricApplicability,
    rating: Option<u8>,
    rating_scale: u8,
    confidence: f64,
    evidence_ids: Vec<EvidenceId>,
}

impl<'de> Deserialize<'de> for RubricJudgment {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data = RubricJudgmentData::deserialize(deserializer)?;
        Self::new(
            data.criterion_id,
            data.applicability,
            data.rating,
            data.rating_scale,
            data.confidence,
            data.evidence_ids,
        )
        .map_err(de::Error::custom)
    }
}

/// A validated set of rubric judgments bound to one evidence bundle.
///
/// The compiler consumes this contract from any provider adapter. The bundle
/// hash records which exact bounded evidence produced the judgments.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RubricJudgmentSet {
    evaluation_version: crate::AnalysisVersion,
    rubric_version: crate::AnalysisVersion,
    status: EvidenceStatus,
    evidence_bundle_hash: ContentHash,
    judgments: Vec<RubricJudgment>,
}

impl RubricJudgmentSet {
    /// Validates and canonicalizes one rubric judgment set.
    ///
    /// A usable status carries judgments; a non-usable status carries none.
    /// Criteria are unique and ordered so compilation is deterministic.
    pub fn new(
        evaluation_version: crate::AnalysisVersion,
        rubric_version: crate::AnalysisVersion,
        status: EvidenceStatus,
        evidence_bundle_hash: ContentHash,
        mut judgments: Vec<RubricJudgment>,
    ) -> Result<Self, DomainValueError> {
        let usable = matches!(status, EvidenceStatus::Complete | EvidenceStatus::Partial);
        if usable == judgments.is_empty() {
            return Err(DomainValueError::new(
                "rubric_judgment_set",
                "a complete or partial set carries judgments and any other status carries none",
            ));
        }
        judgments.sort_by(|left, right| left.criterion_id.cmp(&right.criterion_id));
        if judgments
            .windows(2)
            .any(|pair| pair[0].criterion_id == pair[1].criterion_id)
        {
            return Err(DomainValueError::new(
                "rubric_judgment_set",
                "each criterion may be judged at most once",
            ));
        }
        Ok(Self {
            evaluation_version,
            rubric_version,
            status,
            evidence_bundle_hash,
            judgments,
        })
    }

    /// Returns the Project Intelligence evaluation version boundary.
    pub const fn evaluation_version(&self) -> &crate::AnalysisVersion {
        &self.evaluation_version
    }

    /// Returns the rubric version that produced the judgments.
    pub const fn rubric_version(&self) -> &crate::AnalysisVersion {
        &self.rubric_version
    }

    /// Returns the availability of the judgment set as a whole.
    pub const fn status(&self) -> EvidenceStatus {
        self.status
    }

    /// Returns the content hash of the evidence bundle bound to every citation.
    pub const fn evidence_bundle_hash(&self) -> &ContentHash {
        &self.evidence_bundle_hash
    }

    /// Returns judgments in canonical criterion order.
    pub fn judgments(&self) -> &[RubricJudgment] {
        &self.judgments
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evidence(value: &str) -> EvidenceId {
        EvidenceId::from_str(value).unwrap()
    }

    fn criterion(value: &str) -> RubricCriterionId {
        RubricCriterionId::from_str(value).unwrap()
    }

    #[test]
    fn rejects_rating_above_scale_and_missing_applicable_rating() {
        assert!(
            RubricJudgment::new(
                criterion("substance.claim_implementation_fit"),
                RubricApplicability::Applicable,
                Some(5),
                4,
                0.5,
                vec![evidence("evidence:readme:claim-1")],
            )
            .is_err()
        );
        assert!(
            RubricJudgment::new(
                criterion("substance.claim_implementation_fit"),
                RubricApplicability::Applicable,
                None,
                4,
                0.5,
                vec![evidence("evidence:readme:claim-1")],
            )
            .is_err()
        );
    }

    #[test]
    fn not_applicable_criterion_carries_no_rating_and_needs_no_citation() {
        let judgment = RubricJudgment::new(
            criterion("originality.differentiation"),
            RubricApplicability::NotApplicable,
            None,
            4,
            0.0,
            Vec::new(),
        )
        .unwrap();
        assert_eq!(judgment.rating(), None);
        assert!(judgment.evidence_ids().is_empty());
        assert!(
            RubricJudgment::new(
                criterion("originality.differentiation"),
                RubricApplicability::NotApplicable,
                Some(0),
                4,
                0.0,
                Vec::new(),
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_out_of_range_confidence_and_uncited_applicable_judgment() {
        assert!(
            RubricJudgment::new(
                criterion("substance.claim_implementation_fit"),
                RubricApplicability::Applicable,
                Some(3),
                4,
                1.5,
                vec![evidence("evidence:readme:claim-1")],
            )
            .is_err()
        );
        assert!(
            RubricJudgment::new(
                criterion("substance.claim_implementation_fit"),
                RubricApplicability::PartiallyApplicable,
                Some(2),
                4,
                0.5,
                Vec::new(),
            )
            .is_err()
        );
    }

    #[test]
    fn criterion_dimension_prefix_is_the_leading_segment() {
        assert_eq!(
            criterion("engineering_rigor.tests").dimension_prefix(),
            "engineering_rigor"
        );
    }

    #[test]
    fn judgment_set_status_and_judgment_presence_are_bound() {
        let hash = ContentHash::from_str(&format!("sha256:{}", "a".repeat(64))).unwrap();
        let version = crate::AnalysisVersion::from_str("project-intelligence-1").unwrap();
        let rubric = crate::AnalysisVersion::from_str("project-rubric-1").unwrap();
        assert!(
            RubricJudgmentSet::new(
                version.clone(),
                rubric.clone(),
                EvidenceStatus::Complete,
                hash.clone(),
                Vec::new(),
            )
            .is_err()
        );
        let judgment = RubricJudgment::new(
            criterion("substance.claim_implementation_fit"),
            RubricApplicability::Applicable,
            Some(3),
            4,
            0.75,
            vec![evidence("evidence:readme:claim-1")],
        )
        .unwrap();
        assert!(
            RubricJudgmentSet::new(
                version.clone(),
                rubric.clone(),
                EvidenceStatus::Unavailable,
                hash.clone(),
                vec![judgment.clone()],
            )
            .is_err()
        );
        let set = RubricJudgmentSet::new(
            version,
            rubric,
            EvidenceStatus::Partial,
            hash,
            vec![judgment],
        )
        .unwrap();
        assert_eq!(set.judgments().len(), 1);
    }

    #[test]
    fn judgment_set_rejects_duplicate_criteria() {
        let hash = ContentHash::from_str(&format!("sha256:{}", "b".repeat(64))).unwrap();
        let version = crate::AnalysisVersion::from_str("project-intelligence-1").unwrap();
        let rubric = crate::AnalysisVersion::from_str("project-rubric-1").unwrap();
        let make = || {
            RubricJudgment::new(
                criterion("substance.claim_implementation_fit"),
                RubricApplicability::Applicable,
                Some(3),
                4,
                0.75,
                vec![evidence("evidence:readme:claim-1")],
            )
            .unwrap()
        };
        assert!(
            RubricJudgmentSet::new(
                version,
                rubric,
                EvidenceStatus::Complete,
                hash,
                vec![make(), make()],
            )
            .is_err()
        );
    }
}
