use std::str::FromStr;

use assay_domain::{EvidenceId, RubricApplicability};

use super::classify::classify_project;
use super::observations::{MaturityObservation, TypeObservation};
use super::policy::ClassificationPolicy;
use super::signals::{MaturitySignal, TypeSignal};
use crate::ProjectMaturity;
use crate::ProjectType;
use crate::ScoreDimension;

fn evidence(value: &str) -> EvidenceId {
    EvidenceId::from_str(value).unwrap()
}

fn type_obs(signal: TypeSignal) -> TypeObservation {
    TypeObservation::new(signal, vec![evidence("evidence:readme:signal")]).unwrap()
}

fn maturity_obs(signal: MaturitySignal) -> MaturityObservation {
    MaturityObservation::new(signal, vec![evidence("evidence:history:signal")]).unwrap()
}

#[test]
fn a_single_delivery_form_resolves_to_that_primary_type() {
    let outcome = classify_project(
        &[type_obs(TypeSignal::CliEntrypointDeclared)],
        &[maturity_obs(MaturitySignal::SustainedIteration)],
        &ClassificationPolicy::v1(),
    );
    assert_eq!(outcome.primary_type(), Some(ProjectType::CliDeveloperTool));
    assert_eq!(outcome.maturity(), Some(ProjectMaturity::Prototype));
    assert_eq!(
        outcome.classification().evidence_ids().len(),
        2,
        "the resolved classification cites its type and maturity observations"
    );
}

#[test]
fn a_specific_artifact_type_wins_over_a_delivery_form_and_records_a_secondary() {
    let outcome = classify_project(
        &[
            type_obs(TypeSignal::CuratedListStructure),
            type_obs(TypeSignal::ApplicationEntrypointDeclared),
        ],
        &[maturity_obs(MaturitySignal::StableReleaseTagged)],
        &ClassificationPolicy::v1(),
    );
    assert_eq!(outcome.primary_type(), Some(ProjectType::CuratedResource));
}

#[test]
fn conflicting_delivery_forms_reduce_confidence() {
    let clear = classify_project(
        &[type_obs(TypeSignal::LibraryPackagingDeclared)],
        &[maturity_obs(MaturitySignal::StableReleaseTagged)],
        &ClassificationPolicy::v1(),
    );
    let ambiguous = classify_project(
        &[
            type_obs(TypeSignal::LibraryPackagingDeclared),
            type_obs(TypeSignal::ApplicationEntrypointDeclared),
        ],
        &[maturity_obs(MaturitySignal::StableReleaseTagged)],
        &ClassificationPolicy::v1(),
    );
    assert!(
        ambiguous.classification() != clear.classification(),
        "ambiguity must be reflected in the classification"
    );
}

#[test]
fn an_absent_maturity_signal_yields_an_unknown_classification() {
    let outcome = classify_project(
        &[type_obs(TypeSignal::LibraryPackagingDeclared)],
        &[],
        &ClassificationPolicy::v1(),
    );
    assert_eq!(outcome.primary_type(), None);
    assert_eq!(
        outcome.classification().evidence_ids(),
        &[] as &[EvidenceId],
        "an unknown classification cites nothing and invents no type"
    );
}

#[test]
fn tags_are_derived_from_auxiliary_signals() {
    let outcome = classify_project(
        &[
            type_obs(TypeSignal::LibraryPackagingDeclared),
            type_obs(TypeSignal::FrameworkExtensionPoints),
            type_obs(TypeSignal::PluginHostDeclared),
        ],
        &[maturity_obs(MaturitySignal::StableReleaseTagged)],
        &ClassificationPolicy::v1(),
    );
    assert_eq!(
        outcome.primary_type(),
        Some(ProjectType::LibrarySdkFramework)
    );
}

#[test]
fn a_curated_resource_excludes_engineering_rigor_but_not_via_zero() {
    let outcome = classify_project(
        &[type_obs(TypeSignal::CuratedListStructure)],
        &[maturity_obs(MaturitySignal::MaintenanceModeDeclared)],
        &ClassificationPolicy::v1(),
    );
    assert_eq!(
        outcome.applicability(ScoreDimension::EngineeringRigor),
        RubricApplicability::NotApplicable
    );
    assert_eq!(
        outcome.applicability(ScoreDimension::OpenSourceReadiness),
        RubricApplicability::Applicable
    );
}

#[test]
fn a_young_project_relaxes_rigor_and_maintenance_applicability() {
    let outcome = classify_project(
        &[type_obs(TypeSignal::LibraryPackagingDeclared)],
        &[maturity_obs(MaturitySignal::ConceptOnly)],
        &ClassificationPolicy::v1(),
    );
    assert_eq!(
        outcome.applicability(ScoreDimension::EngineeringRigor),
        RubricApplicability::PartiallyApplicable
    );
    assert_eq!(
        outcome.applicability(ScoreDimension::MaintenanceHealth),
        RubricApplicability::PartiallyApplicable
    );
    assert_eq!(
        outcome.applicability(ScoreDimension::ProjectSubstance),
        RubricApplicability::Applicable
    );
}

#[test]
fn an_unknown_classification_never_marks_a_dimension_not_applicable() {
    let outcome = classify_project(&[], &[], &ClassificationPolicy::v1());
    for dimension in crate::ASSAY_SCORE_DIMENSIONS {
        assert_eq!(
            outcome.applicability(dimension),
            RubricApplicability::PartiallyApplicable
        );
    }
}

#[test]
fn an_uncited_observation_is_rejected() {
    assert_eq!(
        TypeObservation::new(TypeSignal::CliEntrypointDeclared, Vec::new())
            .unwrap_err()
            .kind(),
        super::error::ClassificationErrorKind::UncitedObservation
    );
}
