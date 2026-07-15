use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::Command,
    str::FromStr,
    time::Duration,
};

use assay_domain::{ContentHash, EvidenceStatus, RepositorySource};
use assay_git::{
    CollectionErrorKind, CollectionLimits, CollectionStage, EntryMode, GitCliAdapter,
    GitObjectFormat, HistoryIssue, ObjectIssue, ParentDeltaIssue, RepositorySnapshotPort,
    SnapshotRequest,
};
use assay_test_fixtures::{RepositoryFixture, RepositoryScenario};

fn trusted_git() -> PathBuf {
    for candidate in ["/usr/bin/git", "/usr/local/bin/git"] {
        let path = PathBuf::from(candidate);
        if path.is_file() {
            return path;
        }
    }
    panic!("the Git adapter integration tests require a trusted absolute Git executable");
}

fn source() -> RepositorySource {
    RepositorySource::local(
        ContentHash::from_str(
            "sha256:1111111111111111111111111111111111111111111111111111111111111111",
        )
        .expect("the test repository ID must be valid"),
    )
}

fn adapter(limits: CollectionLimits) -> GitCliAdapter {
    GitCliAdapter::from_trusted_executable(trusted_git(), limits)
        .expect("the installed Git must satisfy the recorded baseline")
}

#[test]
fn collects_an_immutable_snapshot_without_reading_the_working_tree() {
    let fixture = RepositoryFixture::build(RepositoryScenario::TypeScriptProject)
        .expect("the deterministic fixture must build");
    let adapter = adapter(CollectionLimits::default());
    let first = adapter
        .collect(SnapshotRequest::new(
            fixture.path(),
            source(),
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
            source(),
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
    let production = entry(&first, b"src/add.ts");
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
}

#[test]
fn preserves_space_and_unicode_path_bytes() {
    let fixture = RepositoryFixture::build(RepositoryScenario::SpaceAndUnicodePaths)
        .expect("the deterministic fixture must build");
    let snapshot = adapter(CollectionLimits::default())
        .collect(SnapshotRequest::new(
            fixture.path(),
            source(),
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
fn collects_the_same_immutable_revision_from_a_bare_repository() {
    let fixture = RepositoryFixture::build(RepositoryScenario::TypeScriptProject)
        .expect("the deterministic fixture must build");
    let temporary = tempfile::tempdir().expect("the bare repository parent must be creatable");
    let bare = temporary.path().join("fixture.git");
    let status = Command::new(trusted_git())
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
    let snapshot = adapter(CollectionLimits::default())
        .collect(SnapshotRequest::new(&bare, source(), OsStr::new(expected)))
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
    let status = Command::new(trusted_git())
        .current_dir(fixture.path())
        .args(["worktree", "add", "--quiet", "--detach"])
        .arg(&linked)
        .arg("HEAD")
        .status()
        .expect("the synthetic linked-worktree command must start");
    assert!(status.success());

    let snapshot = adapter(CollectionLimits::default())
        .collect(SnapshotRequest::new(&linked, source(), OsStr::new("HEAD")))
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

#[test]
fn shallow_history_and_parent_delta_are_explicitly_partial() {
    let fixture = RepositoryFixture::build(RepositoryScenario::RenameAndMove)
        .expect("the deterministic two-commit fixture must build");
    let temporary = tempfile::tempdir().expect("the shallow clone parent must be creatable");
    let shallow = temporary.path().join("shallow");
    let status = Command::new(trusted_git())
        .args(["clone", "--quiet", "--depth=1", "--no-local"])
        .arg(fixture.path())
        .arg(&shallow)
        .status()
        .expect("the synthetic shallow clone command must start");
    assert!(status.success());

    let snapshot = adapter(CollectionLimits::default())
        .collect(SnapshotRequest::new(&shallow, source(), OsStr::new("HEAD")))
        .expect("a shallow repository must retain usable partial snapshot facts");
    assert_eq!(snapshot.status(), EvidenceStatus::Partial);
    assert_eq!(snapshot.history().status(), EvidenceStatus::Partial);
    assert_eq!(
        snapshot.history().issue(),
        Some(HistoryIssue::ShallowRepository)
    );
    assert!(snapshot.history().truncated());
    assert_eq!(snapshot.parent_delta().status(), EvidenceStatus::Partial);
    assert_eq!(
        snapshot.parent_delta().issue(),
        Some(ParentDeltaIssue::ShallowRepository)
    );
}

#[test]
fn supports_sha256_repositories_with_executable_and_empty_blobs() {
    let temporary = tempfile::tempdir().expect("the SHA-256 repository parent must be creatable");
    let repository = temporary.path().join("sha256-repository");
    let status = Command::new(trusted_git())
        .args([
            "init",
            "--quiet",
            "--initial-branch=main",
            "--object-format=sha256",
        ])
        .arg(&repository)
        .status()
        .expect("the SHA-256 capability probe must start");
    if !status.success() {
        return;
    }
    fs::write(repository.join("empty.bin"), []).expect("the empty blob must be writable");
    fs::write(repository.join("script.sh"), b"#!/bin/sh\nexit 0\n")
        .expect("the non-executed script fixture must be writable");
    run_git(&repository, &[OsStr::new("add"), OsStr::new("--all")]);
    run_git(
        &repository,
        &[
            OsStr::new("update-index"),
            OsStr::new("--chmod=+x"),
            OsStr::new("--"),
            OsStr::new("script.sh"),
        ],
    );
    run_git(
        &repository,
        &[
            OsStr::new("commit"),
            OsStr::new("--quiet"),
            OsStr::new("-m"),
            OsStr::new("Add SHA-256 object fixtures"),
        ],
    );

    let snapshot = adapter(CollectionLimits::default())
        .collect(SnapshotRequest::new(
            &repository,
            source(),
            OsStr::new("HEAD"),
        ))
        .expect("a supported SHA-256 object database must collect consistently");
    assert_eq!(
        snapshot.provenance().object_format(),
        GitObjectFormat::Sha256
    );
    assert_eq!(snapshot.source_snapshot().revision().as_str().len(), 64);
    assert_eq!(
        snapshot
            .source_snapshot()
            .root_tree()
            .unwrap()
            .as_str()
            .len(),
        64
    );
    assert!(
        snapshot
            .entries()
            .iter()
            .all(|entry| entry.object_id().as_str().len() == 64)
    );
    assert_eq!(entry(&snapshot, b"script.sh").mode(), EntryMode::Executable);
    let empty = entry(&snapshot, b"empty.bin");
    assert_eq!(empty.content().size(), Some(0));
    assert_eq!(
        empty.content().content_hash().unwrap().as_str(),
        "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn collects_a_merge_commit_against_its_first_parent() {
    let fixture = RepositoryFixture::build(RepositoryScenario::TypeScriptProject)
        .expect("the deterministic fixture must build");
    run_git(
        fixture.path(),
        &[
            OsStr::new("switch"),
            OsStr::new("--quiet"),
            OsStr::new("-c"),
            OsStr::new("feature"),
        ],
    );
    fs::write(
        fixture.path().join("feature.ts"),
        b"export const feature = true;\n",
    )
    .expect("the feature fixture must be writable");
    run_git(fixture.path(), &[OsStr::new("add"), OsStr::new("--all")]);
    run_git(
        fixture.path(),
        &[
            OsStr::new("commit"),
            OsStr::new("--quiet"),
            OsStr::new("-m"),
            OsStr::new("Add feature branch"),
        ],
    );
    run_git(
        fixture.path(),
        &[
            OsStr::new("switch"),
            OsStr::new("--quiet"),
            OsStr::new("main"),
        ],
    );
    fs::write(
        fixture.path().join("main.ts"),
        b"export const main = true;\n",
    )
    .expect("the main fixture must be writable");
    run_git(fixture.path(), &[OsStr::new("add"), OsStr::new("--all")]);
    run_git(
        fixture.path(),
        &[
            OsStr::new("commit"),
            OsStr::new("--quiet"),
            OsStr::new("-m"),
            OsStr::new("Add main branch"),
        ],
    );
    run_git(
        fixture.path(),
        &[
            OsStr::new("merge"),
            OsStr::new("--quiet"),
            OsStr::new("--no-ff"),
            OsStr::new("-m"),
            OsStr::new("Merge feature"),
            OsStr::new("feature"),
        ],
    );

    let snapshot = adapter(CollectionLimits::default())
        .collect(SnapshotRequest::new(
            fixture.path(),
            source(),
            OsStr::new("HEAD"),
        ))
        .expect("a merge commit must collect with bounded first-parent facts");
    assert_eq!(snapshot.status(), EvidenceStatus::Complete);
    assert_eq!(snapshot.history().reachable_commits(), 4);
    assert_eq!(snapshot.parent_delta().status(), EvidenceStatus::Complete);
    assert_eq!(snapshot.parent_delta().changed_entries(), 1);
}

#[test]
fn accepts_an_empty_immutable_tree_without_fabricating_entries() {
    let fixture = RepositoryFixture::build(RepositoryScenario::MissingReadmeAndLicense)
        .expect("the deterministic fixture must build");
    run_git(
        fixture.path(),
        &[
            OsStr::new("rm"),
            OsStr::new("--quiet"),
            OsStr::new("-r"),
            OsStr::new("--"),
            OsStr::new("."),
        ],
    );
    run_git(
        fixture.path(),
        &[
            OsStr::new("commit"),
            OsStr::new("--quiet"),
            OsStr::new("-m"),
            OsStr::new("Remove all tracked files"),
        ],
    );

    let snapshot = adapter(CollectionLimits::default())
        .collect(SnapshotRequest::new(
            fixture.path(),
            source(),
            OsStr::new("HEAD"),
        ))
        .expect("an empty committed tree remains a valid immutable snapshot");
    assert!(snapshot.entries().is_empty());
    assert_eq!(snapshot.status(), EvidenceStatus::Complete);
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
    run_git(fixture.path(), &[OsStr::new("add"), OsStr::new("--all")]);
    let commit = fixture
        .commit_ids()
        .last()
        .expect("the fixture has a commit");
    run_git(
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
    run_git(
        fixture.path(),
        &[
            OsStr::new("commit"),
            OsStr::new("--quiet"),
            OsStr::new("-m"),
            OsStr::new("Add object edge cases"),
        ],
    );

    let snapshot = adapter(CollectionLimits::default())
        .collect(SnapshotRequest::new(
            fixture.path(),
            source(),
            OsStr::new("HEAD"),
        ))
        .expect("the object edge fixture must collect");
    let raw = snapshot
        .entries()
        .iter()
        .find(|entry| entry.path().as_bytes() == b"invalid-\xff.bin")
        .expect("the invalid UTF-8 path must remain byte-exact");
    assert_eq!(raw.mode(), EntryMode::Regular);
    assert_eq!(raw.content().status(), EvidenceStatus::Complete);

    let link = entry(&snapshot, b"readme-link");
    assert_eq!(link.mode(), EntryMode::SymbolicLink);
    assert_eq!(link.content().status(), EvidenceStatus::Complete);

    let gitlink = entry(&snapshot, b"vendor/submodule");
    assert_eq!(gitlink.mode(), EntryMode::Gitlink);
    assert_eq!(gitlink.content().status(), EvidenceStatus::Unsupported);
    assert_eq!(gitlink.content().issue(), Some(ObjectIssue::GitlinkContent));
    assert_eq!(snapshot.status(), EvidenceStatus::Partial);
}

#[test]
fn applies_explicit_history_rename_object_and_record_limits() {
    let history_fixture = RepositoryFixture::build(RepositoryScenario::RenameAndMove)
        .expect("the deterministic fixture must build");
    let history = adapter(CollectionLimits {
        max_history_commits: 1,
        max_rename_candidates: 2,
        ..CollectionLimits::default()
    })
    .collect(SnapshotRequest::new(
        history_fixture.path(),
        source(),
        OsStr::new("HEAD"),
    ))
    .expect("a bounded history remains a usable snapshot");
    assert_eq!(history.history().status(), EvidenceStatus::Partial);
    assert!(history.history().truncated());
    assert_eq!(history.parent_delta().changed_entries(), 2);
    assert_eq!(history.parent_delta().renames(), 1);

    let rename_limited = adapter(CollectionLimits {
        max_rename_candidates: 1,
        ..CollectionLimits::default()
    })
    .collect(SnapshotRequest::new(
        history_fixture.path(),
        source(),
        OsStr::new("HEAD"),
    ))
    .expect("rename candidate overflow must remain an explicit partial fact");
    assert_eq!(
        rename_limited.parent_delta().status(),
        EvidenceStatus::Partial
    );
    assert_eq!(
        rename_limited.parent_delta().issue(),
        Some(ParentDeltaIssue::RenameCandidateLimit)
    );

    let object_fixture = RepositoryFixture::build(RepositoryScenario::TypeScriptProject)
        .expect("the deterministic fixture must build");
    let object_limited = adapter(CollectionLimits {
        max_object_bytes: 4,
        ..CollectionLimits::default()
    })
    .collect(SnapshotRequest::new(
        object_fixture.path(),
        source(),
        OsStr::new("HEAD"),
    ))
    .expect("oversized blobs must produce partial facts");
    assert_eq!(object_limited.status(), EvidenceStatus::Partial);
    assert!(object_limited.entries().iter().all(|entry| {
        entry.content().status() == EvidenceStatus::Partial
            && entry.content().issue() == Some(ObjectIssue::SizeLimit)
            && entry.content().content_hash().is_none()
    }));

    let record_error = adapter(CollectionLimits {
        max_tree_entries: 1,
        ..CollectionLimits::default()
    })
    .collect(SnapshotRequest::new(
        object_fixture.path(),
        source(),
        OsStr::new("HEAD"),
    ))
    .expect_err("tree record overflow must fail closed");
    assert_eq!(record_error.stage(), CollectionStage::EnumerateTree);
    assert_eq!(record_error.kind(), CollectionErrorKind::RecordLimit);
}

#[test]
fn treats_a_revision_beginning_with_dash_as_an_operand() {
    let fixture = RepositoryFixture::build(RepositoryScenario::TypeScriptProject)
        .expect("the deterministic fixture must build");
    let error = adapter(CollectionLimits::default())
        .collect(SnapshotRequest::new(
            fixture.path(),
            source(),
            OsStr::new("--help"),
        ))
        .expect_err("an option-shaped revision must not become an option");

    assert_eq!(error.stage(), CollectionStage::ResolveRevision);
    assert_eq!(error.kind(), CollectionErrorKind::NonZeroExit);
    assert!(!format!("{error:?}").contains(fixture.path().to_string_lossy().as_ref()));
    assert!(!format!("{error}").contains("--help"));
}

#[test]
fn reports_missing_and_incompatible_git_without_executable_paths() {
    let missing = GitCliAdapter::from_trusted_executable(
        PathBuf::from("/definitely/missing/assay-git"),
        CollectionLimits::default(),
    )
    .expect_err("a missing executable must fail capability probing");
    assert_eq!(missing.stage(), CollectionStage::ProbeCapabilities);
    assert_eq!(missing.kind(), CollectionErrorKind::ExecutableMissing);
    assert!(!format!("{missing:?}").contains("/definitely"));
}

fn entry<'a>(
    snapshot: &'a assay_git::RepositorySnapshot,
    path: &[u8],
) -> &'a assay_git::TrackedEntry {
    snapshot
        .entries()
        .iter()
        .find(|entry| entry.path().as_bytes() == path)
        .expect("the expected tracked entry must exist")
}

fn run_git(repository: &Path, arguments: &[&OsStr]) {
    let status = Command::new(trusted_git())
        .current_dir(repository)
        .env("GIT_AUTHOR_NAME", "Assay Edge Author")
        .env("GIT_AUTHOR_EMAIL", "edge-author@example.invalid")
        .env("GIT_AUTHOR_DATE", "2001-02-05T06:07:08+09:00")
        .env("GIT_COMMITTER_NAME", "Assay Edge Committer")
        .env("GIT_COMMITTER_EMAIL", "edge-committer@example.invalid")
        .env("GIT_COMMITTER_DATE", "2001-02-05T06:07:08+09:00")
        .args(arguments)
        .status()
        .expect("the synthetic Git command must start");
    assert!(status.success(), "the synthetic Git command must succeed");
}

#[allow(dead_code)]
fn short_timeout_limits() -> CollectionLimits {
    CollectionLimits {
        command_timeout: Duration::from_millis(100),
        ..CollectionLimits::default()
    }
}

#[allow(dead_code)]
fn parent_limit_issue_is_public() -> ParentDeltaIssue {
    ParentDeltaIssue::RenameCandidateLimit
}
