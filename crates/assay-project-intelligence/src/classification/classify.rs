use std::collections::BTreeMap;

use assay_domain::{EvidenceId, EvidenceStatus};

use crate::ProjectClassification;
use crate::ProjectMaturity;
use crate::ProjectType;
use crate::classification::applicability::{criteria_applicability, unknown_applicability};
use crate::classification::observations::{MaturityObservation, TypeObservation};
use crate::classification::outcome::ClassificationOutcome;
use crate::classification::policy::ClassificationPolicy;
use crate::classification::signals::{
    DELIVERY_FORM_TYPES, MATURITY_PRIORITY, MaturitySignal, TYPE_PRIORITY,
};

/// Classifies a project from cited type and maturity observations.
///
/// A usable classification requires both a resolved type and a resolved
/// maturity; otherwise the result is an explicit unknown classification with an
/// `Insufficient` status and no invented type, maturity, or zero.
pub fn classify_project(
    type_observations: &[TypeObservation],
    maturity_observations: &[MaturityObservation],
    policy: &ClassificationPolicy,
) -> ClassificationOutcome {
    let (primary_type, secondary_types, type_evidence, type_ambiguous) =
        resolve_type(type_observations);
    let tags = resolve_tags(type_observations);
    let (maturity, maturity_evidence) = resolve_maturity(maturity_observations);

    match (primary_type, maturity) {
        (Some(primary), Some(maturity)) => {
            let confidence = classification_confidence(policy, type_ambiguous);
            let evidence_ids =
                sorted_unique(type_evidence.into_iter().chain(maturity_evidence).collect());
            let classification = ProjectClassification::new(
                EvidenceStatus::Complete,
                Some(primary),
                secondary_types,
                tags,
                Some(maturity),
                confidence,
                evidence_ids,
            )
            .expect("resolved observations satisfy the classification contract");
            ClassificationOutcome {
                classification,
                primary_type: Some(primary),
                maturity: Some(maturity),
                applicability: criteria_applicability(primary, maturity, policy),
                policy_version: policy.policy_version,
                applicability_policy_version: policy.applicability_policy_version,
            }
        }
        _ => {
            let classification = ProjectClassification::new(
                EvidenceStatus::Insufficient,
                None,
                Vec::new(),
                Vec::new(),
                None,
                0.0,
                Vec::new(),
            )
            .expect("an unknown classification satisfies the classification contract");
            ClassificationOutcome {
                classification,
                primary_type: None,
                maturity: None,
                applicability: unknown_applicability(),
                policy_version: policy.policy_version,
                applicability_policy_version: policy.applicability_policy_version,
            }
        }
    }
}

fn resolve_type(
    observations: &[TypeObservation],
) -> (Option<ProjectType>, Vec<ProjectType>, Vec<EvidenceId>, bool) {
    let mut matched: BTreeMap<ProjectType, Vec<EvidenceId>> = BTreeMap::new();
    for observation in observations {
        if let Some(project_type) = observation.signal.primary_type() {
            matched
                .entry(project_type)
                .or_default()
                .extend(observation.evidence_ids.iter().cloned());
        }
    }
    let Some(primary) = TYPE_PRIORITY
        .into_iter()
        .find(|candidate| matched.contains_key(candidate))
    else {
        return (None, Vec::new(), Vec::new(), false);
    };
    let mut secondary_types = matched
        .keys()
        .copied()
        .filter(|candidate| *candidate != primary)
        .collect::<Vec<_>>();
    secondary_types.sort();
    let evidence = matched.into_values().flatten().collect();
    let delivery_forms = secondary_types
        .iter()
        .chain(std::iter::once(&primary))
        .filter(|candidate| DELIVERY_FORM_TYPES.contains(candidate))
        .count();
    (Some(primary), secondary_types, evidence, delivery_forms > 1)
}

fn resolve_tags(observations: &[TypeObservation]) -> Vec<String> {
    let mut tags = observations
        .iter()
        .filter_map(|observation| observation.signal.tag())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    tags.sort();
    tags.dedup();
    tags
}

fn resolve_maturity(
    observations: &[MaturityObservation],
) -> (Option<ProjectMaturity>, Vec<EvidenceId>) {
    let mut matched: BTreeMap<MaturitySignal, Vec<EvidenceId>> = BTreeMap::new();
    for observation in observations {
        matched
            .entry(observation.signal)
            .or_default()
            .extend(observation.evidence_ids.iter().cloned());
    }
    let Some(signal) = MATURITY_PRIORITY
        .into_iter()
        .find(|candidate| matched.contains_key(candidate))
    else {
        return (None, Vec::new());
    };
    let evidence = matched
        .remove(&signal)
        .expect("the selected signal is present");
    (Some(signal.maturity()), evidence)
}

fn classification_confidence(policy: &ClassificationPolicy, ambiguous: bool) -> f64 {
    let type_bp = if ambiguous {
        policy.ambiguous_type_confidence_bp
    } else {
        policy.single_type_confidence_bp
    };
    f64::from((type_bp + policy.maturity_confidence_bp) / 2) / 10_000.0
}

fn sorted_unique(mut ids: Vec<EvidenceId>) -> Vec<EvidenceId> {
    ids.sort();
    ids.dedup();
    ids
}
