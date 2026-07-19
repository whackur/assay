#[cfg(unix)]
use assay_classifier::LinguistAttributeFacts;
#[cfg(unix)]
use assay_domain::{AnalysisStatus, EvidenceStatus};
#[cfg(unix)]
use assay_project_intelligence::ClassifiedSnapshotFile;
#[cfg(unix)]
use assay_project_intelligence::{
    ClassificationAvailabilityReason, EvidenceAssemblyErrorKind, PortablePathEncoding,
    RawEvidenceIssue, RawEvidenceKind, assemble_project_evidence, build_project_analysis,
};

mod evidence_manifest_helpers;
#[cfg(unix)]
use evidence_manifest_helpers::{NamedPolicy, classifications, edge_snapshot, related_ids};

#[cfg(unix)]
#[test]
fn unavailable_path_only_feature_cites_non_utf8_raw_evidence() {
    let snapshot = edge_snapshot();
    let manifest = assemble_project_evidence(
        &snapshot,
        classifications(&snapshot, LinguistAttributeFacts::available(None, None)),
    )
    .unwrap();
    let expected = manifest
        .raw_facts()
        .iter()
        .filter(|fact| {
            fact.kind() == RawEvidenceKind::TrackedFile
                && fact
                    .source()
                    .path()
                    .is_some_and(|path| path.encoding() != PortablePathEncoding::Utf8)
        })
        .map(|fact| fact.id().as_str().to_owned())
        .collect::<Vec<_>>();
    let output = build_project_analysis(&snapshot, &manifest, "2026-01-02T03:04:06Z").unwrap();
    let license = output["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .find(|fact| fact["payload"]["feature"] == "license")
        .unwrap();
    assert_eq!(license["payload"]["state"], "unavailable");
    assert!(!expected.is_empty());
    assert_eq!(related_ids(license), expected);
    for id in related_ids(license) {
        let cited = output["evidence"]
            .as_array()
            .unwrap()
            .iter()
            .find(|fact| fact["id"] == id)
            .unwrap();
        assert_eq!(cited["payload"]["kind"], "tracked_file");
    }
}

#[cfg(unix)]
#[test]
fn unsupported_classifications_participate_in_single_policy_enforcement_and_identity() {
    let snapshot = edge_snapshot();
    let normal = snapshot
        .entries()
        .iter()
        .find(|entry| std::str::from_utf8(entry.path().as_bytes()).is_ok())
        .unwrap();
    let unsupported = snapshot
        .entries()
        .iter()
        .filter(|entry| std::str::from_utf8(entry.path().as_bytes()).is_err())
        .collect::<Vec<_>>();
    assert_eq!(unsupported.len(), 2);

    let classify = |entry, version| {
        ClassifiedSnapshotFile::classify(
            &snapshot,
            entry,
            LinguistAttributeFacts::available(None, None),
            &NamedPolicy(version),
        )
        .unwrap()
    };

    for facts in [
        vec![
            classify(normal, "test-policy-1"),
            classify(unsupported[0], "test-policy-2"),
        ],
        vec![
            classify(unsupported[0], "test-policy-2"),
            classify(normal, "test-policy-1"),
        ],
        vec![
            classify(unsupported[0], "test-policy-1"),
            classify(unsupported[1], "test-policy-2"),
        ],
        vec![
            classify(unsupported[1], "test-policy-2"),
            classify(unsupported[0], "test-policy-1"),
        ],
    ] {
        assert_eq!(
            assemble_project_evidence(&snapshot, facts)
                .unwrap_err()
                .kind(),
            EvidenceAssemblyErrorKind::MixedClassificationPolicy
        );
    }

    let same_policy = assemble_project_evidence(
        &snapshot,
        unsupported
            .iter()
            .map(|entry| classify(entry, "test-policy-1")),
    )
    .unwrap();
    let unsupported_records = same_policy
        .classification_facts()
        .iter()
        .filter(|fact| fact.reason() == Some(ClassificationAvailabilityReason::NonPortablePath))
        .collect::<Vec<_>>();
    assert_eq!(unsupported_records.len(), 2);
    assert!(unsupported_records.iter().all(|fact| {
        fact.status() == EvidenceStatus::Unsupported
            && fact.policy_version() == Some("test-policy-1")
    }));
    assert_eq!(
        same_policy.classification_policy_version(),
        Some("test-policy-1")
    );

    let one_policy = |version| {
        assemble_project_evidence(&snapshot, [classify(unsupported[0], version)]).unwrap()
    };
    let first = one_policy("test-policy-1");
    let second = one_policy("test-policy-2");
    let unsupported_id = |manifest: &assay_project_intelligence::ProjectEvidenceManifest| {
        manifest
            .classification_facts()
            .iter()
            .find(|fact| {
                fact.reason() == Some(ClassificationAvailabilityReason::NonPortablePath)
                    && fact.policy_version().is_some()
            })
            .unwrap()
            .id()
            .clone()
    };
    assert_ne!(unsupported_id(&first), unsupported_id(&second));
}

#[cfg(unix)]
#[test]
fn non_utf8_path_and_gitlink_remain_citable_unsupported_facts() {
    let snapshot = edge_snapshot();
    let manifest = assemble_project_evidence(
        &snapshot,
        classifications(&snapshot, LinguistAttributeFacts::available(None, None)),
    )
    .unwrap();

    let gitlink = manifest.raw_facts().iter().find(|fact| {
        fact.payload()
            .as_tracked_file()
            .is_some_and(|payload| payload.issue() == Some(RawEvidenceIssue::GitlinkContent))
    });
    assert_eq!(gitlink.unwrap().status(), EvidenceStatus::Unsupported);
    let unsupported_path = manifest
        .classification_facts()
        .iter()
        .find(|fact| fact.reason() == Some(ClassificationAvailabilityReason::NonPortablePath))
        .expect("non-UTF-8 path must remain explicit");
    assert_eq!(unsupported_path.status(), EvidenceStatus::Unsupported);
    assert_eq!(
        unsupported_path.source().path().unwrap().encoding(),
        PortablePathEncoding::GitPathHex
    );
    let duplicate_blob_facts = manifest
        .raw_facts()
        .iter()
        .filter(|fact| {
            fact.kind() == RawEvidenceKind::TrackedFile
                && matches!(
                    fact.source().path().map(|path| path.value()),
                    Some("src/copy-a.ts" | "src/copy-b.ts")
                )
        })
        .collect::<Vec<_>>();
    assert_eq!(duplicate_blob_facts.len(), 2);
    assert_eq!(
        duplicate_blob_facts[0].source().object_id(),
        duplicate_blob_facts[1].source().object_id()
    );
    assert_ne!(duplicate_blob_facts[0].id(), duplicate_blob_facts[1].id());
    assert_eq!(manifest.status(), AnalysisStatus::Partial);
}
