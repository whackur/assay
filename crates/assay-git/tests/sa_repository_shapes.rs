mod snapshot_adapter_helpers;

use assay_domain::EvidenceStatus;
use assay_git::{RepositorySnapshotPort, SnapshotRequest};
use assay_test_fixtures::{RepositoryFixture, RepositoryScenario};
use snapshot_adapter_helpers as helpers;
#[cfg(unix)]
use std::fs;
use std::{ffi::OsStr, process::Command};

#[test]
fn collects_the_same_immutable_revision_from_a_bare_repository() {
    let fixture = RepositoryFixture::build(RepositoryScenario::TypeScriptProject)
        .expect("the deterministic fixture must build");
    let temporary = tempfile::tempdir().expect("the bare repository parent must be creatable");
    let bare = temporary.path().join("fixture.git");
    let status = Command::new(helpers::trusted_git())
        .args([
            OsStr::new("clone"),
            OsStr::new("--bare"),
            OsStr::new("--no-hardlinks"),
        ])
        .arg("--quiet")
        .arg(fixture.path())
        .arg(&bare)
        .status()
        .expect("the synthetic bare clone command must start");
    assert!(status.success());

    let expected = fixture
        .commit_ids()
        .last()
        .expect("the fixture has a commit");
    let snapshot = helpers::adapter(helpers::default_limits())
        .collect(SnapshotRequest::new(
            &bare,
            helpers::source(),
            OsStr::new(expected),
        ))
        .expect("a bare object database must collect without a working tree");
    assert_eq!(snapshot.source_snapshot().revision().as_str(), expected);
    assert_eq!(snapshot.status(), EvidenceStatus::Complete);
}

#[test]
fn accepts_a_genuine_linked_worktree_with_matching_backlinks() {
    let fixture = RepositoryFixture::build(RepositoryScenario::TypeScriptProject)
        .expect("the deterministic fixture must build");
    let linked = fixture
        .path()
        .parent()
        .expect("the fixture has a temporary parent")
        .join("linked-worktree");
    let status = Command::new(helpers::trusted_git())
        .current_dir(fixture.path())
        .args(["worktree", "add", "--quiet", "--detach"])
        .arg(&linked)
        .arg("HEAD")
        .status()
        .expect("the synthetic linked-worktree command must start");
    assert!(status.success());

    let snapshot = helpers::adapter(helpers::default_limits())
        .collect(SnapshotRequest::new(
            &linked,
            helpers::source(),
            OsStr::new("HEAD"),
        ))
        .expect("a genuine linked worktree must satisfy pointer and backlink validation");
    assert_eq!(snapshot.status(), EvidenceStatus::Complete);
    assert_eq!(
        snapshot.source_snapshot().revision().as_str(),
        fixture
            .commit_ids()
            .last()
            .expect("the fixture has a commit")
    );
}

#[cfg(unix)]
#[test]
fn preserves_invalid_utf8_and_handles_symlink_gitlink_and_binary_blob() {
    use std::os::unix::{ffi::OsStringExt, fs::symlink};

    let fixture = RepositoryFixture::build(RepositoryScenario::TypeScriptProject)
        .expect("the deterministic fixture must build");
    let raw_name = std::ffi::OsString::from_vec(b"invalid-\xff.bin".to_vec());
    fs::write(fixture.path().join(&raw_name), b"binary\0payload\xff")
        .expect("the raw-byte file must be writable");
    symlink("README.md", fixture.path().join("readme-link"))
        .expect("the synthetic symlink must be creatable");
    helpers::run_git(fixture.path(), &[OsStr::new("add"), OsStr::new("--all")]);
    let commit = fixture
        .commit_ids()
        .last()
        .expect("the fixture has a commit");
    helpers::run_git(
        fixture.path(),
        &[
            OsStr::new("update-index"),
            OsStr::new("--add"),
            OsStr::new("--cacheinfo"),
            OsStr::new("160000"),
            OsStr::new(commit),
            OsStr::new("vendor/submodule"),
        ],
    );
    helpers::run_git(
        fixture.path(),
        &[
            OsStr::new("commit"),
            OsStr::new("--quiet"),
            OsStr::new("-m"),
            OsStr::new("Add object edge cases"),
        ],
    );

    let snapshot = helpers::adapter(helpers::default_limits())
        .collect(SnapshotRequest::new(
            fixture.path(),
            helpers::source(),
            OsStr::new("HEAD"),
        ))
        .expect("the object edge fixture must collect");
    let raw = snapshot
        .entries()
        .iter()
        .find(|entry| entry.path().as_bytes() == b"invalid-\xff.bin")
        .expect("the invalid UTF-8 path must remain byte-exact");
    assert_eq!(raw.mode(), assay_git::EntryMode::Regular);
    assert_eq!(raw.content().status(), EvidenceStatus::Complete);

    let link = helpers::entry(&snapshot, b"readme-link");
    assert_eq!(link.mode(), assay_git::EntryMode::SymbolicLink);
    assert_eq!(link.content().status(), EvidenceStatus::Complete);

    let gitlink = helpers::entry(&snapshot, b"vendor/submodule");
    assert_eq!(gitlink.mode(), assay_git::EntryMode::Gitlink);
    assert_eq!(gitlink.content().status(), EvidenceStatus::Unsupported);
    assert_eq!(
        gitlink.content().issue(),
        Some(assay_git::ObjectIssue::GitlinkContent)
    );
    assert_eq!(snapshot.status(), EvidenceStatus::Partial);
}
