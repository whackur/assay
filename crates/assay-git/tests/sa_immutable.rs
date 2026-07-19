mod snapshot_adapter_helpers;

use assay_domain::EvidenceStatus;
use assay_git::{RepositorySnapshotPort, SnapshotRequest};
use assay_test_fixtures::{RepositoryFixture, RepositoryScenario};
use snapshot_adapter_helpers as helpers;
use std::{ffi::OsStr, fs};

#[test]
fn collects_an_immutable_snapshot_without_reading_the_working_tree() {
    let fixture = RepositoryFixture::build(RepositoryScenario::TypeScriptProject)
        .expect("the deterministic fixture must build");
    let adapter = helpers::adapter(helpers::default_limits());
    let first = adapter
        .collect(SnapshotRequest::new(
            fixture.path(),
            helpers::source(),
            OsStr::new("HEAD"),
        ))
        .expect("the immutable snapshot must collect");

    fs::remove_file(fixture.path().join("src/add.ts"))
        .expect("the working-tree file must be removable");
    fs::write(
        fixture.path().join("README.md"),
        b"working tree bytes that are not committed\n",
    )
    .expect("the working-tree file must be replaceable");

    let second = adapter
        .collect(SnapshotRequest::new(
            fixture.path(),
            helpers::source(),
            OsStr::new("HEAD"),
        ))
        .expect("collection must use the immutable object database");

    assert_eq!(first, second);
    assert_eq!(first.status(), EvidenceStatus::Complete);
    assert_eq!(
        first.source_snapshot().revision().as_str(),
        fixture
            .commit_ids()
            .last()
            .expect("the fixture has a commit")
    );
    assert!(first.source_snapshot().root_tree().is_some());
    assert_eq!(first.entries().len(), 5);
    assert!(first.entries().iter().all(|entry| {
        entry.content().status() == EvidenceStatus::Complete
            && entry.content().size().is_some()
            && entry.content().content_hash().is_some()
    }));
    let production = helpers::entry(&first, b"src/add.ts");
    assert_eq!(production.content().size(), Some(84));
    assert_eq!(
        production
            .content()
            .content_hash()
            .expect("the bounded blob must have a digest")
            .as_str(),
        "sha256:a968c8f2bbfa307017a7a4af8f5fe13762891e3fa754fc495b9c1d80a460f073"
    );
    assert_eq!(first.history().reachable_commits(), 1);
    assert!(!first.history().truncated());
    assert_eq!(first.parent_delta().changed_entries(), 0);
    assert_eq!(first.provenance().adapter_id(), "installed-git-cli-v1");
    assert!(!first.provenance().git_version().is_empty());
    assert_eq!(first.commit_time(), "2001-02-02T19:05:06Z");
}

#[test]
fn preserves_space_and_unicode_path_bytes() {
    let fixture = RepositoryFixture::build(RepositoryScenario::SpaceAndUnicodePaths)
        .expect("the deterministic fixture must build");
    let snapshot = helpers::adapter(helpers::default_limits())
        .collect(SnapshotRequest::new(
            fixture.path(),
            helpers::source(),
            OsStr::new("HEAD"),
        ))
        .expect("the Unicode fixture must collect");
    let paths = snapshot
        .entries()
        .iter()
        .map(|entry| entry.path().as_bytes())
        .collect::<Vec<_>>();

    assert!(paths.contains(&"docs/résumé.md".as_bytes()));
    assert!(paths.contains(&b"src/hello world.ts".as_slice()));
}

#[test]
fn accepts_an_empty_immutable_tree_without_fabricating_entries() {
    let fixture = RepositoryFixture::build(RepositoryScenario::MissingReadmeAndLicense)
        .expect("the deterministic fixture must build");
    helpers::run_git(
        fixture.path(),
        &[
            OsStr::new("rm"),
            OsStr::new("--quiet"),
            OsStr::new("-r"),
            OsStr::new("--"),
            OsStr::new("."),
        ],
    );
    helpers::run_git(
        fixture.path(),
        &[
            OsStr::new("commit"),
            OsStr::new("--quiet"),
            OsStr::new("-m"),
            OsStr::new("Remove all tracked files"),
        ],
    );

    let snapshot = helpers::adapter(helpers::default_limits())
        .collect(SnapshotRequest::new(
            fixture.path(),
            helpers::source(),
            OsStr::new("HEAD"),
        ))
        .expect("an empty committed tree remains a valid immutable snapshot");
    assert!(snapshot.entries().is_empty());
    assert_eq!(snapshot.status(), EvidenceStatus::Complete);
}
