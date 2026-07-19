#![cfg(unix)]

mod security_boundaries_helpers;

use assay_domain::EvidenceStatus;
use assay_git::{
    CollectionErrorKind, CollectionLimits, CollectionStage, GitCliAdapter, ObjectIssue,
    ParentDeltaIssue, RepositorySnapshotPort,
};
use assay_test_fixtures::{RepositoryFixture, RepositoryScenario};
use security_boundaries_helpers as helpers;
use serial_test::serial;

#[test]
#[serial]
fn rejects_mixed_object_ids_and_impossible_raw_statuses() {
    let mixed_tree = r#"
for argument in "$@"; do
  if [ "$argument" = "ls-tree" ]; then
    printf '100644 blob 1111111111111111111111111111111111111111111111111111111111111111\tfile\000'
    exit 0
  fi
done
exec /usr/bin/git "$@"
"#;
    let (_directory, mixed_git) = helpers::wrapper(mixed_tree);
    let fixture = helpers::fixture();
    let adapter = GitCliAdapter::from_trusted_executable(mixed_git, CollectionLimits::default())
        .expect("the mixed-ID wrapper must pass the capability probe");
    let error = adapter
        .collect(helpers::request(fixture.path()))
        .expect_err("a SHA-256 tree ID inside a SHA-1 repository must fail closed");
    assert_eq!(error.stage(), CollectionStage::EnumerateTree);
    assert_eq!(error.kind(), CollectionErrorKind::MalformedOutput);

    let impossible_add = r#"
for argument in "$@"; do
  if [ "$argument" = "diff-tree" ]; then
    printf ':100644 100644 1111111111111111111111111111111111111111 2222222222222222222222222222222222222222 A\000path\000'
    exit 0
  fi
done
exec /usr/bin/git "$@"
"#;
    let (_directory, impossible_git) = helpers::wrapper(impossible_add);
    let fixture = RepositoryFixture::build(RepositoryScenario::RenameAndMove)
        .expect("the rename fixture must build");
    let adapter =
        GitCliAdapter::from_trusted_executable(impossible_git, CollectionLimits::default())
            .expect("the impossible-status wrapper must pass the capability probe");
    let snapshot = adapter
        .collect(helpers::request(fixture.path()))
        .expect("invalid optional delta evidence must leave a partial snapshot");
    assert_eq!(
        snapshot.parent_delta().status(),
        EvidenceStatus::Unavailable
    );
    assert_eq!(
        snapshot.parent_delta().issue(),
        Some(ParentDeltaIssue::MalformedOutput)
    );
}

#[test]
#[serial]
fn raw_diff_contract_rejects_invalid_type_status_score_and_copy_records() {
    const OLD: &str = "1111111111111111111111111111111111111111";
    const NEW: &str = "2222222222222222222222222222222222222222";
    const NULL: &str = "0000000000000000000000000000000000000000";
    let fixture = RepositoryFixture::build(RepositoryScenario::RenameAndMove)
        .expect("the rename fixture must build");
    let empty = "";
    let malformed = [
        (
            "chmod reported as a type change",
            format!(":100644 100755 {OLD} {NEW} T\\000path\\000"),
            empty.to_owned(),
        ),
        (
            "type change reported as a modification",
            format!(":100644 120000 {OLD} {NEW} M\\000path\\000"),
            empty.to_owned(),
        ),
        (
            "rename emitted while detection is disabled",
            format!(":100644 100644 {OLD} {NEW} R100\\000old\\000new\\000"),
            empty.to_owned(),
        ),
        (
            "rename across object type classes",
            empty.to_owned(),
            format!(":100644 120000 {OLD} {NEW} R100\\000old\\000new\\000"),
        ),
        (
            "rename below the requested threshold",
            empty.to_owned(),
            format!(":100644 100644 {OLD} {NEW} R049\\000old\\000new\\000"),
        ),
        (
            "unrequested copy detection",
            empty.to_owned(),
            format!(":100644 100644 {OLD} {NEW} C100\\000old\\000new\\000"),
        ),
        (
            "present add source",
            format!(":100644 100644 {OLD} {NEW} A\\000path\\000"),
            empty.to_owned(),
        ),
        (
            "absent modification target",
            format!(":100644 000000 {OLD} {NULL} M\\000path\\000"),
            empty.to_owned(),
        ),
    ];

    for (case, no_renames, find_renames) in malformed {
        let snapshot = helpers::collect_with_raw_diff(fixture.path(), &no_renames, &find_renames);
        assert_eq!(
            snapshot.parent_delta().status(),
            EvidenceStatus::Unavailable,
            "{case}"
        );
        assert_eq!(
            snapshot.parent_delta().issue(),
            Some(ParentDeltaIssue::MalformedOutput),
            "{case}"
        );
    }
}

#[test]
#[serial]
fn raw_diff_contract_accepts_chmod_type_change_and_threshold_renames() {
    const OLD: &str = "1111111111111111111111111111111111111111";
    const NEW: &str = "2222222222222222222222222222222222222222";
    const NULL: &str = "0000000000000000000000000000000000000000";
    let fixture = RepositoryFixture::build(RepositoryScenario::RenameAndMove)
        .expect("the rename fixture must build");

    for (case, record) in [
        (
            "regular-file chmod",
            format!(":100644 100755 {OLD} {NEW} M\\000path\\000"),
        ),
        (
            "regular-file to symlink type change",
            format!(":100644 120000 {OLD} {NEW} T\\000path\\000"),
        ),
    ] {
        let snapshot = helpers::collect_with_raw_diff(fixture.path(), &record, &record);
        assert_eq!(
            snapshot.parent_delta().status(),
            EvidenceStatus::Complete,
            "{case}"
        );
        assert_eq!(snapshot.parent_delta().changed_entries(), 1, "{case}");
        assert_eq!(snapshot.parent_delta().renames(), 0, "{case}");
    }

    let delete_add = format!(
        ":100644 000000 {OLD} {NULL} D\\000old\\000:000000 100644 {NULL} {NEW} A\\000new\\000"
    );
    for score in ["050", "100"] {
        let rename = format!(":100644 100755 {OLD} {NEW} R{score}\\000old\\000new\\000");
        let snapshot = helpers::collect_with_raw_diff(fixture.path(), &delete_add, &rename);
        assert_eq!(
            snapshot.parent_delta().status(),
            EvidenceStatus::Complete,
            "R{score}"
        );
        assert_eq!(snapshot.parent_delta().changed_entries(), 2, "R{score}");
        assert_eq!(snapshot.parent_delta().renames(), 1, "R{score}");
    }
}

#[test]
#[serial]
fn malformed_raw_diff_is_an_explicit_unavailable_parent_delta() {
    let script = r#"
for argument in "$@"; do
  if [ "$argument" = "diff-tree" ]; then
    printf ':bogus header R100\000old\000new\000'
    exit 0
  fi
done
exec /usr/bin/git "$@"
"#;
    let (_directory, malformed_git) = helpers::wrapper(script);
    let fixture = RepositoryFixture::build(RepositoryScenario::RenameAndMove)
        .expect("the rename fixture must build");
    let adapter =
        GitCliAdapter::from_trusted_executable(malformed_git, CollectionLimits::default())
            .expect("the malformed diff wrapper must pass the capability probe");
    let snapshot = adapter
        .collect(helpers::request(fixture.path()))
        .expect("malformed optional delta evidence must preserve the source snapshot");

    assert_eq!(snapshot.status(), EvidenceStatus::Partial);
    assert_eq!(
        snapshot.parent_delta().status(),
        EvidenceStatus::Unavailable
    );
    assert_eq!(
        snapshot.parent_delta().issue(),
        Some(assay_git::ParentDeltaIssue::MalformedOutput)
    );
}

#[allow(dead_code)]
fn _ensure_object_issue_import_used() -> ObjectIssue {
    ObjectIssue::MissingOrUnreadable
}
