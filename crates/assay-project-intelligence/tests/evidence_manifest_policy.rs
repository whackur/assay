use assay_classifier::LinguistAttributeFacts;
use assay_domain::EvidenceStatus;
use assay_git::CollectionLimits;
use assay_project_intelligence::{
    ClassificationEvidenceRecord, ClassifiedSnapshotFile, EvidenceAssemblyErrorKind,
    HistoryScopeEvidence, ParentDeltaEvidence, RawEvidenceIssue, TrackedFileEvidence,
    assemble_project_evidence,
};
use assay_test_fixtures::RepositoryScenario;

mod evidence_manifest_helpers;
use evidence_manifest_helpers::{NamedPolicy, classifications, snapshot};

#[test]
fn mixed_present_policy_versions_fail_closed_but_missing_records_do_not_invent_a_policy() {
    let snapshot = snapshot(
        RepositoryScenario::TypeScriptProject,
        CollectionLimits::default(),
    );
    for versions in [
        ["test-policy-1", "test-policy-2"],
        ["test-policy-2", "test-policy-1"],
    ] {
        let facts = snapshot
            .entries()
            .iter()
            .take(2)
            .zip(versions)
            .map(|(entry, version)| {
                ClassifiedSnapshotFile::classify(
                    &snapshot,
                    entry,
                    LinguistAttributeFacts::available(None, None),
                    &NamedPolicy(version),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let error = assemble_project_evidence(&snapshot, facts).unwrap_err();
        assert_eq!(
            error.kind(),
            EvidenceAssemblyErrorKind::MixedClassificationPolicy
        );
    }

    let one = vec![
        ClassifiedSnapshotFile::classify(
            &snapshot,
            &snapshot.entries()[0],
            LinguistAttributeFacts::available(None, None),
            &NamedPolicy("test-policy-1"),
        )
        .unwrap(),
    ];
    let partial = assemble_project_evidence(&snapshot, one).unwrap();
    assert_eq!(
        partial.classification_policy_version(),
        Some("test-policy-1")
    );
    assert!(partial.classification_facts().iter().skip(1).any(|fact| {
        fact.status() == EvidenceStatus::Unavailable && fact.policy_version().is_none()
    }));
}

#[test]
fn downstream_can_name_public_records_and_read_safe_payload_views() {
    fn classification(record: &ClassificationEvidenceRecord) -> Option<&str> {
        record.policy_version()
    }
    fn tracked(view: TrackedFileEvidence<'_>) -> Option<RawEvidenceIssue> {
        let _ = (
            view.mode(),
            view.object_kind(),
            view.size_bytes(),
            view.content_hash(),
        );
        view.issue()
    }
    fn history(view: HistoryScopeEvidence<'_>) -> Option<RawEvidenceIssue> {
        let _ = (view.reachable_commits(), view.truncated());
        view.issue()
    }
    fn delta(view: ParentDeltaEvidence<'_>) -> Option<RawEvidenceIssue> {
        let _ = (view.changed_entries(), view.renames());
        view.issue()
    }

    let snapshot = snapshot(
        RepositoryScenario::TypeScriptProject,
        CollectionLimits::default(),
    );
    let manifest = assemble_project_evidence(
        &snapshot,
        classifications(&snapshot, LinguistAttributeFacts::available(None, None)),
    )
    .unwrap();
    assert!(
        manifest
            .classification_facts()
            .iter()
            .all(|record| classification(record) == Some("file-classifier-1"))
    );
    for fact in manifest.raw_facts() {
        if let Some(view) = fact.payload().as_tracked_file() {
            let _ = tracked(view);
        }
        if let Some(view) = fact.payload().as_history_scope() {
            let _ = history(view);
        }
        if let Some(view) = fact.payload().as_parent_delta() {
            let _ = delta(view);
        }
    }
}

#[test]
fn classification_facts_retain_rule_confidence_and_attribute_provenance() {
    let snapshot = snapshot(
        RepositoryScenario::TypeScriptProject,
        CollectionLimits::default(),
    );
    let manifest = assemble_project_evidence(
        &snapshot,
        classifications(
            &snapshot,
            LinguistAttributeFacts::available(Some(false), Some(false)),
        ),
    )
    .unwrap();

    for fact in manifest.classification_facts() {
        assert_eq!(fact.policy_version(), Some("file-classifier-1"));
        assert!(fact.category().is_some());
        assert!(fact.rule_id().is_some());
        assert!(fact.confidence_basis_points().is_some());
        assert!(!fact.classification_evidence().is_empty());
        assert!(fact.source().path().is_some());
        assert!(fact.source().object_id().is_some());
    }
}

#[test]
fn classification_evidence_ids_change_across_policy_availability_but_raw_ids_do_not() {
    let snapshot = snapshot(
        RepositoryScenario::TypeScriptProject,
        CollectionLimits::default(),
    );
    let complete = assemble_project_evidence(
        &snapshot,
        classifications(&snapshot, LinguistAttributeFacts::available(None, None)),
    )
    .unwrap();
    let partial = assemble_project_evidence(
        &snapshot,
        classifications(&snapshot, LinguistAttributeFacts::unavailable()),
    )
    .unwrap();

    assert_eq!(
        complete
            .raw_facts()
            .iter()
            .map(|fact| fact.id().as_str())
            .collect::<Vec<_>>(),
        partial
            .raw_facts()
            .iter()
            .map(|fact| fact.id().as_str())
            .collect::<Vec<_>>()
    );
    assert_ne!(
        complete
            .classification_facts()
            .iter()
            .map(|fact| fact.id().as_str())
            .collect::<Vec<_>>(),
        partial
            .classification_facts()
            .iter()
            .map(|fact| fact.id().as_str())
            .collect::<Vec<_>>()
    );
}
