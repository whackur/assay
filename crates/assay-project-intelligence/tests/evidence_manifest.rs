use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    str::FromStr,
};
#[cfg(unix)]
use std::{fs, process::Command};

use assay_classifier::{
    BuiltInPolicy, ClassificationDecision, ClassificationPolicy, FileClassificationInput,
    LinguistAttributeFacts, PolicyVersion,
};
use assay_domain::{AnalysisStatus, ContentHash, EvidenceStatus, RepositorySource};
use assay_git::{
    CollectionLimits, GitCliAdapter, RepositorySnapshot, RepositorySnapshotPort, SnapshotRequest,
};
use assay_project_intelligence::{
    ClassificationAvailabilityReason, ClassificationCategoryRecord, ClassificationEvidenceRecord,
    ClassifiedSnapshotFile, EvidenceAssemblyErrorKind, HistoryScopeEvidence, ParentDeltaEvidence,
    PortablePathEncoding, RawEvidenceIssue, RawEvidenceKind, TrackedFileEvidence,
    assemble_project_evidence, build_project_analysis,
};
use assay_test_fixtures::{RepositoryFixture, RepositoryScenario, trusted_git_executable};

fn trusted_git() -> PathBuf {
    trusted_git_executable().expect("tests require a deployment-trusted Git executable")
}

#[test]
fn complete_attribute_facts_allow_absent_features_without_false_unavailability() {
    let snapshot = snapshot(
        RepositoryScenario::MissingReadmeAndLicense,
        CollectionLimits::default(),
    );
    let manifest = assemble_project_evidence(
        &snapshot,
        classifications(&snapshot, LinguistAttributeFacts::available(None, None)),
    )
    .unwrap();
    let output = build_project_analysis(&snapshot, &manifest, "2026-01-02T03:04:06Z").unwrap();
    assert!(
        output["manifest"]["limitations"]
            .as_array()
            .unwrap()
            .iter()
            .all(|limitation| limitation["code"] != "attribute_resolution_unavailable"),
        "complete attribute facts must not claim unavailable resolution"
    );
    let features = output["evidence"].as_array().unwrap();
    for feature in ["readme", "license", "generated_content", "vendored_content"] {
        let fact = features
            .iter()
            .find(|fact| fact["payload"]["feature"] == feature)
            .expect("feature fact must exist");
        assert_eq!(fact["payload"]["state"], "absent");
        assert_eq!(fact["status"], "complete");
    }
}

#[test]
fn repository_features_cite_the_evidence_layer_that_supports_each_claim() {
    let snapshot = snapshot(
        RepositoryScenario::TypeScriptProject,
        CollectionLimits::default(),
    );
    let complete = assemble_project_evidence(
        &snapshot,
        classifications(&snapshot, LinguistAttributeFacts::available(None, None)),
    )
    .unwrap();
    let output = build_project_analysis(&snapshot, &complete, "2026-01-02T03:04:06Z").unwrap();
    let readme_raw_ids = complete
        .raw_facts()
        .iter()
        .filter(|fact| {
            fact.kind() == RawEvidenceKind::TrackedFile
                && fact.source().path().is_some_and(|path| {
                    path.encoding() == PortablePathEncoding::Utf8
                        && path
                            .value()
                            .to_ascii_lowercase()
                            .rsplit('/')
                            .next()
                            .is_some_and(|name| name.starts_with("readme"))
                })
        })
        .map(|fact| fact.id().as_str().to_owned())
        .collect::<Vec<_>>();
    let test_classification_ids = complete
        .classification_facts()
        .iter()
        .filter(|fact| fact.category() == Some(ClassificationCategoryRecord::Test))
        .map(|fact| fact.id().as_str().to_owned())
        .collect::<Vec<_>>();
    let features = output["evidence"].as_array().unwrap();
    let readme = features
        .iter()
        .find(|fact| fact["payload"]["feature"] == "readme")
        .unwrap();
    assert!(!readme_raw_ids.is_empty());
    assert_eq!(related_ids(readme), readme_raw_ids);
    let test_feature = features
        .iter()
        .find(|fact| fact["payload"]["feature"] == "test")
        .unwrap();
    assert_eq!(test_feature["payload"]["state"], "present");
    assert!(!test_classification_ids.is_empty());
    assert_eq!(related_ids(test_feature), test_classification_ids);

    let partial = assemble_project_evidence(
        &snapshot,
        classifications(&snapshot, LinguistAttributeFacts::unavailable()),
    )
    .unwrap();
    let partial_output =
        build_project_analysis(&snapshot, &partial, "2026-01-02T03:04:06Z").unwrap();
    let partial_test_ids = partial
        .classification_facts()
        .iter()
        .filter(|fact| fact.category() == Some(ClassificationCategoryRecord::Test))
        .map(|fact| fact.id().as_str().to_owned())
        .collect::<Vec<_>>();
    let partial_test = partial_output["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .find(|fact| fact["payload"]["feature"] == "test")
        .unwrap();
    assert_eq!(partial_test["payload"]["state"], "unavailable");
    assert!(!partial_test_ids.is_empty());
    assert_eq!(related_ids(partial_test), partial_test_ids);
}

#[test]
fn unavailable_no_match_feature_cites_all_incomplete_classifications_and_policy_identity() {
    let snapshot = snapshot(
        RepositoryScenario::TypeScriptProject,
        CollectionLimits::default(),
    );
    let analyze = |version| {
        let facts = snapshot
            .entries()
            .iter()
            .map(|entry| {
                ClassifiedSnapshotFile::classify(
                    &snapshot,
                    entry,
                    LinguistAttributeFacts::unavailable(),
                    &NamedPolicy(version),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let manifest = assemble_project_evidence(&snapshot, facts).unwrap();
        let expected = manifest
            .classification_facts()
            .iter()
            .map(|fact| fact.id().as_str().to_owned())
            .collect::<Vec<_>>();
        let output = build_project_analysis(&snapshot, &manifest, "2026-01-02T03:04:06Z").unwrap();
        (output, expected)
    };
    let (first, first_expected) = analyze("test-policy-1");
    let (second, second_expected) = analyze("test-policy-2");
    let first_feature = feature(&first, "generated_content");
    let second_feature = feature(&second, "generated_content");
    assert_eq!(first_feature["payload"]["state"], "unavailable");
    assert!(!first_expected.is_empty());
    assert_eq!(related_ids(first_feature), first_expected);
    assert_eq!(related_ids(second_feature), second_expected);
    assert_ne!(first_feature["id"], second_feature["id"]);
    for id in related_ids(first_feature) {
        let cited = first["evidence"]
            .as_array()
            .unwrap()
            .iter()
            .find(|fact| fact["id"] == id)
            .unwrap();
        assert_eq!(cited["payload"]["kind"], "file_classification");
    }
}

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

fn related_ids(feature: &serde_json::Value) -> Vec<String> {
    feature["payload"]["related_evidence_ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|id| id.as_str().unwrap().to_owned())
        .collect()
}

fn feature<'a>(output: &'a serde_json::Value, name: &str) -> &'a serde_json::Value {
    output["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .find(|fact| fact["payload"]["feature"] == name)
        .unwrap()
}

#[test]
fn repository_feature_identity_includes_classification_policy_identity() {
    let snapshot = snapshot(
        RepositoryScenario::TypeScriptProject,
        CollectionLimits::default(),
    );
    let analyze = |version| {
        let facts = snapshot
            .entries()
            .iter()
            .map(|entry| {
                ClassifiedSnapshotFile::classify(
                    &snapshot,
                    entry,
                    LinguistAttributeFacts::available(None, None),
                    &NamedPolicy(version),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let manifest = assemble_project_evidence(&snapshot, facts).unwrap();
        build_project_analysis(&snapshot, &manifest, "2026-01-02T03:04:06Z").unwrap()
    };
    let first = analyze("test-policy-1");
    let second = analyze("test-policy-2");
    let feature_id = |value: &serde_json::Value| {
        value["evidence"]
            .as_array()
            .unwrap()
            .iter()
            .find(|fact| fact["payload"]["feature"] == "test")
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    assert_ne!(feature_id(&first), feature_id(&second));
}

#[test]
fn missing_classification_mapper_omits_an_unattempted_policy() {
    let snapshot = snapshot(
        RepositoryScenario::MissingReadmeAndLicense,
        CollectionLimits::default(),
    );
    let manifest = assemble_project_evidence(&snapshot, []).unwrap();
    let output = build_project_analysis(&snapshot, &manifest, "2026-01-02T03:04:06Z").unwrap();
    let missing = output["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .find(|fact| fact["reason"] == "missing_classification")
        .expect("missing classification envelope");
    assert!(missing.get("attempted_policy_version").is_none());
    assert_eq!(missing["requested_kind"], "file_classification");
    assert_eq!(missing["related_evidence_ids"].as_array().unwrap().len(), 1);
}

fn source_with_digest(digit: char) -> RepositorySource {
    RepositorySource::local(
        ContentHash::from_str(&format!("sha256:{}", digit.to_string().repeat(64))).unwrap(),
    )
}

fn source() -> RepositorySource {
    source_with_digest('1')
}

fn snapshot(scenario: RepositoryScenario, limits: CollectionLimits) -> RepositorySnapshot {
    let fixture = RepositoryFixture::build(scenario).expect("fixture must build");
    collect_snapshot(fixture.path(), OsStr::new("HEAD"), source(), limits)
}

fn collect_snapshot(
    repository: &Path,
    revision: &OsStr,
    source: RepositorySource,
    limits: CollectionLimits,
) -> RepositorySnapshot {
    GitCliAdapter::from_trusted_executable(trusted_git(), limits)
        .expect("Git must satisfy the adapter baseline")
        .collect(SnapshotRequest::new(repository, source, revision))
        .expect("snapshot must collect")
}

fn classifications(
    snapshot: &RepositorySnapshot,
    attributes: LinguistAttributeFacts,
) -> Vec<ClassifiedSnapshotFile> {
    snapshot
        .entries()
        .iter()
        .map(|entry| {
            ClassifiedSnapshotFile::classify(snapshot, entry, attributes, &BuiltInPolicy::V1)
                .unwrap()
        })
        .collect()
}

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

    let assemble = |snapshot: &RepositorySnapshot| {
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

struct NamedPolicy(&'static str);

impl ClassificationPolicy for NamedPolicy {
    fn policy_version(&self) -> PolicyVersion {
        PolicyVersion::try_new(self.0).unwrap()
    }

    fn evaluate(&self, input: &FileClassificationInput) -> ClassificationDecision {
        BuiltInPolicy::V1.evaluate(input)
    }
}

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

#[cfg(unix)]
fn edge_snapshot() -> RepositorySnapshot {
    use std::os::unix::ffi::OsStringExt;

    let directory = tempfile::tempdir().unwrap();
    let repository = directory.path().join("edge-repository");
    git(
        directory.path(),
        [
            "init",
            "--quiet",
            "--initial-branch=main",
            repository.to_str().unwrap(),
        ],
    );
    git(&repository, ["config", "user.name", "Assay Fixture"]);
    git(
        &repository,
        ["config", "user.email", "fixture@example.invalid"],
    );
    fs::write(repository.join("README.md"), b"# Edge fixture\n").unwrap();
    for (name, contents) in [
        (b"src/non-utf8-\xfe.ts".as_slice(), b"first\n".as_slice()),
        (b"src/non-utf8-\xff.ts".as_slice(), b"second\n".as_slice()),
    ] {
        let invalid_path = repository.join(std::ffi::OsString::from_vec(name.to_vec()));
        fs::create_dir_all(invalid_path.parent().unwrap()).unwrap();
        fs::write(invalid_path, contents).unwrap();
    }
    fs::write(
        repository.join("src/copy-a.ts"),
        b"export const duplicated = true;\n",
    )
    .unwrap();
    fs::write(
        repository.join("src/copy-b.ts"),
        b"export const duplicated = true;\n",
    )
    .unwrap();
    git(&repository, ["add", "--all", "--", "."]);
    git(&repository, ["commit", "--quiet", "-m", "Add edge paths"]);
    let target = git_output(&repository, ["rev-parse", "HEAD"]);
    git(
        &repository,
        [
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{target},deps/module"),
        ],
    );
    git(&repository, ["commit", "--quiet", "-m", "Add gitlink"]);
    collect_snapshot(
        &repository,
        OsStr::new("HEAD"),
        source(),
        CollectionLimits::default(),
    )
}

#[cfg(unix)]
fn git<const N: usize>(directory: &Path, arguments: [&str; N]) {
    let status = Command::new(trusted_git())
        .current_dir(directory)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .args(arguments)
        .status()
        .unwrap();
    assert!(status.success());
}

#[cfg(unix)]
fn git_output<const N: usize>(directory: &Path, arguments: [&str; N]) -> String {
    let output = Command::new(trusted_git())
        .current_dir(directory)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .args(arguments)
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
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
        .find(|fact| fact.status() == EvidenceStatus::Unavailable)
        .expect("missing classification must remain a citable envelope");
    assert_eq!(
        missing_fact.reason(),
        Some(ClassificationAvailabilityReason::MissingClassification)
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
    assert_eq!(snapshot.status(), EvidenceStatus::Partial);

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
            .all(|fact| fact.status() == EvidenceStatus::Partial)
    );
    assert!(
        manifest
            .classification_facts()
            .iter()
            .all(|fact| fact.status() == EvidenceStatus::Partial)
    );
    assert!(manifest.classification_facts().iter().all(|fact| {
        fact.reason() == Some(ClassificationAvailabilityReason::AttributesUnavailable)
    }));
    assert!(
        manifest
            .raw_facts()
            .iter()
            .filter(|fact| fact.kind() == RawEvidenceKind::TrackedFile)
            .all(|fact| fact.content_hash().is_none())
    );
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
