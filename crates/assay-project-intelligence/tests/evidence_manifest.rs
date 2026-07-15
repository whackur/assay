use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::Command,
    str::FromStr,
};

use assay_classifier::{
    BuiltInPolicy, ClassificationDecision, ClassificationPolicy, FileClassificationInput,
    LinguistAttributeFacts, PolicyVersion,
};
use assay_domain::{AnalysisStatus, ContentHash, EvidenceStatus, RepositorySource};
use assay_git::{
    CollectionLimits, GitCliAdapter, RepositorySnapshot, RepositorySnapshotPort, SnapshotRequest,
};
use assay_project_intelligence::{
    ClassificationAvailabilityReason, ClassifiedSnapshotFile, EvidenceAssemblyErrorKind,
    PortablePathEncoding, RawEvidenceIssue, RawEvidenceKind, RawEvidencePayload,
    assemble_project_evidence,
};
use assay_test_fixtures::{RepositoryFixture, RepositoryScenario};

fn trusted_git() -> PathBuf {
    ["/usr/bin/git", "/usr/local/bin/git"]
        .into_iter()
        .map(PathBuf::from)
        .find(|path| path.is_file())
        .expect("tests require a deployment-trusted Git executable")
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
    let invalid_name = std::ffi::OsString::from_vec(b"src/non-utf8-\xff.ts".to_vec());
    let invalid_path = repository.join(invalid_name);
    fs::create_dir_all(invalid_path.parent().unwrap()).unwrap();
    fs::write(&invalid_path, b"export const edge = true;\n").unwrap();
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

    let snapshot = collect_snapshot(
        &repository,
        OsStr::new("HEAD"),
        source(),
        CollectionLimits::default(),
    );
    let manifest = assemble_project_evidence(
        &snapshot,
        classifications(&snapshot, LinguistAttributeFacts::available(None, None)),
    )
    .unwrap();

    let gitlink = manifest.raw_facts().iter().find(|fact| {
        matches!(
            fact.payload(),
            RawEvidencePayload::TrackedFile {
                issue: Some(RawEvidenceIssue::GitlinkContent),
                ..
            }
        )
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
