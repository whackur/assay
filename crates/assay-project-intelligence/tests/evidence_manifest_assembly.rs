use std::ffi::OsStr;

use assay_classifier::LinguistAttributeFacts;
use assay_domain::AnalysisStatus;
use assay_git::CollectionLimits;
use assay_project_intelligence::{
    EvidenceAssemblyErrorKind, RawEvidenceKind, assemble_project_evidence,
};
use assay_test_fixtures::{RepositoryFixture, RepositoryScenario};

mod evidence_manifest_helpers;
use evidence_manifest_helpers::{classifications, collect_snapshot, snapshot, source_with_digest};

#[test]
fn assembles_canonical_raw_and_classification_evidence_without_scores_or_people() {
    let snapshot = snapshot(
        RepositoryScenario::TypeScriptProject,
        CollectionLimits::default(),
    );
    let manifest = assemble_project_evidence(
        &snapshot,
        classifications(&snapshot, LinguistAttributeFacts::available(None, None)),
    )
    .expect("matching facts must assemble");

    assert_eq!(manifest.status(), AnalysisStatus::Complete);
    assert_eq!(manifest.raw_facts().len(), snapshot.entries().len() + 3);
    assert_eq!(
        manifest.classification_facts().len(),
        snapshot.entries().len()
    );
    assert!(
        manifest
            .raw_facts()
            .windows(2)
            .all(|pair| pair[0].id() < pair[1].id())
    );
    assert!(
        manifest
            .classification_facts()
            .windows(2)
            .all(|pair| pair[0].id() < pair[1].id())
    );

    for raw in manifest.raw_facts() {
        assert_eq!(
            raw.source().repository_revision(),
            snapshot.source_snapshot().revision()
        );
        assert_eq!(
            raw.source().root_tree(),
            snapshot.source_snapshot().root_tree()
        );
        assert_eq!(
            raw.source().provenance().adapter_id(),
            snapshot.provenance().adapter_id()
        );
        assert!(!format!("{raw:?}").contains("target/assay-fixtures"));
    }
    assert!(
        manifest
            .raw_facts()
            .iter()
            .filter(|fact| fact.kind() == RawEvidenceKind::TrackedFile)
            .all(|fact| fact.source().object_id().is_some())
    );
}

#[test]
fn evidence_ids_ignore_input_order() {
    let snapshot = snapshot(
        RepositoryScenario::TypeScriptProject,
        CollectionLimits::default(),
    );
    let forward = classifications(&snapshot, LinguistAttributeFacts::available(None, None));
    let mut reverse = forward.clone();
    reverse.reverse();

    let first = assemble_project_evidence(&snapshot, forward).unwrap();
    let second = assemble_project_evidence(&snapshot, reverse).unwrap();

    assert_eq!(first, second);
    assert!(
        first
            .all_evidence_ids()
            .all(|id| id.as_str().starts_with("evidence:"))
    );
}

#[test]
fn source_revision_and_evidence_kind_are_domain_separated_in_ids() {
    let fixture = RepositoryFixture::build(RepositoryScenario::DependencyOnlyChange)
        .expect("fixture must build");
    let first_revision = OsStr::new(&fixture.commit_ids()[0]);
    let second_revision = OsStr::new(&fixture.commit_ids()[1]);
    let first = collect_snapshot(
        fixture.path(),
        first_revision,
        source_with_digest('1'),
        CollectionLimits::default(),
    );
    let different_source = collect_snapshot(
        fixture.path(),
        first_revision,
        source_with_digest('2'),
        CollectionLimits::default(),
    );
    let different_revision = collect_snapshot(
        fixture.path(),
        second_revision,
        source_with_digest('1'),
        CollectionLimits::default(),
    );

    let assemble = |snapshot: &assay_git::RepositorySnapshot| {
        assemble_project_evidence(
            snapshot,
            classifications(snapshot, LinguistAttributeFacts::available(None, None)),
        )
        .unwrap()
    };
    let first = assemble(&first);
    let different_source = assemble(&different_source);
    let different_revision = assemble(&different_revision);

    assert_ne!(
        first.raw_facts()[0].id(),
        different_source.raw_facts()[0].id()
    );
    assert_ne!(
        first.raw_facts()[0].id(),
        different_revision.raw_facts()[0].id()
    );
    assert!(
        first
            .all_evidence_ids()
            .all(|id| id.as_str().contains(":v1-"))
    );
    let raw_ids = first
        .raw_facts()
        .iter()
        .map(|fact| fact.id().as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert!(
        first
            .classification_facts()
            .iter()
            .all(|fact| !raw_ids.contains(fact.id().as_str()))
    );
}

#[test]
fn classification_binding_rejects_same_path_and_object_from_another_source_scope() {
    let fixture = RepositoryFixture::build(RepositoryScenario::TypeScriptProject)
        .expect("fixture must build");
    let first = collect_snapshot(
        fixture.path(),
        OsStr::new("HEAD"),
        source_with_digest('1'),
        CollectionLimits::default(),
    );
    let second = collect_snapshot(
        fixture.path(),
        OsStr::new("HEAD"),
        source_with_digest('2'),
        CollectionLimits::default(),
    );
    assert_eq!(first.entries(), second.entries());
    let foreign = classifications(&first, LinguistAttributeFacts::available(None, None));
    let error = assemble_project_evidence(&second, foreign).unwrap_err();
    assert_eq!(
        error.kind(),
        EvidenceAssemblyErrorKind::ClassificationSnapshotMismatch
    );
}

#[test]
fn missing_is_citable_while_duplicate_and_foreign_classifications_fail_closed() {
    let target_snapshot = snapshot(
        RepositoryScenario::MissingReadmeAndLicense,
        CollectionLimits::default(),
    );
    let mut facts = classifications(
        &target_snapshot,
        LinguistAttributeFacts::available(None, None),
    );

    let missing = facts.pop().unwrap();
    let manifest = assemble_project_evidence(&target_snapshot, facts.clone()).unwrap();
    assert_eq!(manifest.status(), AnalysisStatus::Partial);
    let missing_fact = manifest
        .classification_facts()
        .iter()
        .find(|fact| fact.status() == assay_domain::EvidenceStatus::Unavailable)
        .expect("missing classification must remain a citable envelope");
    assert_eq!(
        missing_fact.reason(),
        Some(assay_project_intelligence::ClassificationAvailabilityReason::MissingClassification)
    );
    assert!(missing_fact.source_evidence_id().is_some());

    facts.push(missing.clone());
    facts.push(missing);
    let duplicate = assemble_project_evidence(&target_snapshot, facts).unwrap_err();
    assert_eq!(
        duplicate.kind(),
        EvidenceAssemblyErrorKind::DuplicateClassification
    );

    let other_snapshot = snapshot(
        RepositoryScenario::SpaceAndUnicodePaths,
        CollectionLimits::default(),
    );
    let foreign = classifications(
        &other_snapshot,
        LinguistAttributeFacts::available(None, None),
    );
    let foreign_error = assemble_project_evidence(&target_snapshot, foreign).unwrap_err();
    assert_eq!(
        foreign_error.kind(),
        EvidenceAssemblyErrorKind::ClassificationSnapshotMismatch
    );
    let rendered = format!("{foreign_error:?} {foreign_error}");
    assert!(!rendered.contains("src/"));
    assert!(!rendered.contains("docs/"));
    assert!(!rendered.contains("sha256:"));
}

#[test]
fn partial_content_and_unavailable_attributes_propagate_without_zero_substitution() {
    let limits = CollectionLimits {
        max_object_bytes: 1,
        ..CollectionLimits::default()
    };
    let snapshot = snapshot(RepositoryScenario::TypeScriptProject, limits);
    assert_eq!(snapshot.status(), assay_domain::EvidenceStatus::Partial);

    let manifest = assemble_project_evidence(
        &snapshot,
        classifications(&snapshot, LinguistAttributeFacts::unavailable()),
    )
    .unwrap();

    assert_eq!(manifest.status(), AnalysisStatus::Partial);
    assert!(
        manifest
            .raw_facts()
            .iter()
            .filter(|fact| fact.kind() == RawEvidenceKind::TrackedFile)
            .all(|fact| fact.status() == assay_domain::EvidenceStatus::Partial)
    );
    assert!(
        manifest
            .classification_facts()
            .iter()
            .all(|fact| fact.status() == assay_domain::EvidenceStatus::Partial)
    );
    assert!(manifest.classification_facts().iter().all(|fact| {
        fact.reason()
            == Some(
                assay_project_intelligence::ClassificationAvailabilityReason::AttributesUnavailable,
            )
    }));
    assert!(
        manifest
            .raw_facts()
            .iter()
            .filter(|fact| fact.kind() == RawEvidenceKind::TrackedFile)
            .all(|fact| fact.content_hash().is_none())
    );
}
