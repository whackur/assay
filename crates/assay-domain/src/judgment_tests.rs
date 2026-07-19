use std::str::FromStr;

use crate::{
    AnalysisVersion, ContentHash, EvidenceId, EvidenceStatus, RubricApplicability,
    RubricCriterionId, RubricJudgment, RubricJudgmentSet,
};

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
    let version = AnalysisVersion::from_str("project-intelligence-1").unwrap();
    let rubric = AnalysisVersion::from_str("project-rubric-1").unwrap();
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
    let version = AnalysisVersion::from_str("project-intelligence-1").unwrap();
    let rubric = AnalysisVersion::from_str("project-rubric-1").unwrap();
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
