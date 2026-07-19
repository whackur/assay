//! ADP-001: `ProjectEvidenceManifest` -> `EvidenceBundle` adapter contract.
//!
//! The adapter bridges the manifest produced by `assay-project-intelligence`
//! and the bundle consumed by `assay-ai-evaluator`. It must preserve evidence
//! identifiers, keep the privacy scope and transmission policy explicit, never
//! expose raw source or host paths, and be deterministic.

use std::str::FromStr;

use assay_ai_evaluator::{
    AdapterPrivacy, DeterministicFakeProvider, Evaluator, EvidenceKind, EvidenceScope,
    ExternalTransmission, QualitativeRubric, manifest_to_evidence_bundle,
};
use assay_classifier::{BuiltInPolicy, LinguistAttributeFacts};
use assay_git::{CollectionLimits, GitCliAdapter, RepositorySnapshotPort, SnapshotRequest};
use assay_project_intelligence::assemble_project_evidence;
use assay_test_fixtures::{RepositoryFixture, RepositoryScenario, trusted_git_executable};
use std::ffi::OsStr;

fn trusted_git() -> std::path::PathBuf {
    trusted_git_executable().expect("tests require a deployment-trusted Git executable")
}

fn snapshot(scenario: RepositoryScenario) -> assay_git::RepositorySnapshot {
    let fixture = RepositoryFixture::build(scenario).expect("fixture must build");
    let adapter =
        GitCliAdapter::from_trusted_executable(trusted_git(), CollectionLimits::default())
            .expect("Git must satisfy the adapter baseline");
    let source = assay_domain::RepositorySource::local(
        assay_domain::ContentHash::from_str(&format!("sha256:{}", "1".repeat(64))).unwrap(),
    );
    adapter
        .collect(SnapshotRequest::new(
            fixture.path(),
            source,
            OsStr::new("HEAD"),
        ))
        .expect("snapshot must collect")
}

fn manifest(scenario: RepositoryScenario) -> assay_project_intelligence::ProjectEvidenceManifest {
    let snapshot = snapshot(scenario);
    let classifications = snapshot
        .entries()
        .iter()
        .map(|entry| {
            assay_project_intelligence::ClassifiedSnapshotFile::classify(
                &snapshot,
                entry,
                LinguistAttributeFacts::unavailable(),
                &BuiltInPolicy::V1,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    assemble_project_evidence(&snapshot, classifications).expect("manifest must assemble")
}

#[test]
fn adapter_preserves_evidence_ids_in_canonical_order() {
    let manifest = manifest(RepositoryScenario::TypeScriptProject);
    let bundle = manifest_to_evidence_bundle(&manifest, AdapterPrivacy::local_deterministic())
        .expect("adapter must produce a bundle");

    let manifest_ids = manifest
        .all_evidence_ids()
        .map(|id| id.as_str().to_owned())
        .collect::<Vec<_>>();
    let bundle_ids = bundle
        .items()
        .iter()
        .map(|item| item.id().as_str().to_owned())
        .collect::<Vec<_>>();
    let mut expected = manifest_ids.clone();
    expected.sort();
    assert_eq!(bundle_ids, expected);
}

#[test]
fn adapter_is_deterministic_for_identical_manifests() {
    let manifest = manifest(RepositoryScenario::TypeScriptProject);
    let first = manifest_to_evidence_bundle(&manifest, AdapterPrivacy::local_deterministic())
        .expect("first bundle");
    let second = manifest_to_evidence_bundle(&manifest, AdapterPrivacy::local_deterministic())
        .expect("second bundle");
    assert_eq!(first.content_hash(), second.content_hash());
    assert_eq!(first.items().len(), second.items().len());
}

#[test]
fn adapter_never_exposes_raw_source_or_host_paths() {
    let manifest = manifest(RepositoryScenario::TypeScriptProject);
    let bundle = manifest_to_evidence_bundle(&manifest, AdapterPrivacy::local_deterministic())
        .expect("adapter must produce a bundle");
    let text = format!("{bundle:?}");
    assert!(!text.contains("return left + right"));
    assert!(!text.contains("Synthetic repository evidence"));
    assert!(!text.contains("target/assay-fixtures"));
    for item in bundle.items() {
        let statement = item.statement();
        assert!(!statement.contains("return left + right"));
        assert!(!statement.contains("Synthetic repository evidence"));
        assert!(!statement.contains("target/assay-fixtures"));
    }
}

#[test]
fn adapter_carries_local_deterministic_privacy() {
    let manifest = manifest(RepositoryScenario::TypeScriptProject);
    let bundle = manifest_to_evidence_bundle(&manifest, AdapterPrivacy::local_deterministic())
        .expect("adapter must produce a bundle");
    assert_eq!(bundle.scope(), EvidenceScope::PrivateLocal);
    assert_eq!(bundle.transmission(), ExternalTransmission::NotUsed);
}

#[test]
fn adapter_output_is_consumable_by_the_deterministic_evaluator() {
    let manifest = manifest(RepositoryScenario::TypeScriptProject);
    let bundle = manifest_to_evidence_bundle(&manifest, AdapterPrivacy::local_deterministic())
        .expect("adapter must produce a bundle");
    let evaluator = Evaluator::new(QualitativeRubric::project_v1());
    let judgments = evaluator
        .evaluate(&DeterministicFakeProvider::valid(), &bundle)
        .expect("the deterministic provider must accept the adapted bundle");
    assert_eq!(judgments.evidence_bundle_hash(), bundle.content_hash());
    assert_eq!(judgments.judgments().len(), 4);
    for judgment in judgments.judgments() {
        for citation in judgment.evidence_ids() {
            assert!(
                bundle.items().iter().any(|item| item.id() == citation),
                "a judgment citation must stay inside the adapted bundle"
            );
        }
    }
}

#[test]
fn adapter_maps_raw_kinds_to_evidence_kinds() {
    let manifest = manifest(RepositoryScenario::TypeScriptProject);
    let bundle = manifest_to_evidence_bundle(&manifest, AdapterPrivacy::local_deterministic())
        .expect("adapter must produce a bundle");
    let kinds = bundle
        .items()
        .iter()
        .map(|item| item.kind())
        .collect::<std::collections::BTreeSet<_>>();
    assert!(kinds.contains(&EvidenceKind::RepositoryFact));
    assert!(kinds.contains(&EvidenceKind::ImplementationFact));
}
