mod snapshot_adapter_helpers;

use assay_domain::EvidenceStatus;
use assay_git::{
    CollectionErrorKind, CollectionLimits, CollectionStage, GitObjectFormat, HistoryIssue,
    ObjectIssue, ParentDeltaIssue, RepositorySnapshotPort, SnapshotRequest,
};
use assay_test_fixtures::{RepositoryFixture, RepositoryScenario};
use snapshot_adapter_helpers as helpers;
use std::{ffi::OsStr, fs, process::Command};

#[test]
fn shallow_history_and_parent_delta_are_explicitly_partial() {
    let fixture = RepositoryFixture::build(RepositoryScenario::RenameAndMove)
        .expect("the deterministic two-commit fixture must build");
    let temporary = tempfile::tempdir().expect("the shallow clone parent must be creatable");
    let shallow = temporary.path().join("shallow");
    let status = Command::new(helpers::trusted_git())
        .args(["clone", "--quiet", "--depth=1", "--no-local"])
        .arg(fixture.path())
        .arg(&shallow)
        .status()
        .expect("the synthetic shallow clone command must start");
    assert!(status.success());

    let snapshot = helpers::adapter(helpers::default_limits())
        .collect(SnapshotRequest::new(
            &shallow,
            helpers::source(),
            OsStr::new("HEAD"),
        ))
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
fn applies_explicit_history_rename_object_and_record_limits() {
    let history_fixture = RepositoryFixture::build(RepositoryScenario::RenameAndMove)
        .expect("the deterministic fixture must build");
    let history = helpers::adapter(CollectionLimits {
        max_history_commits: 1,
        max_rename_candidates: 2,
        ..CollectionLimits::default()
    })
    .collect(SnapshotRequest::new(
        history_fixture.path(),
        helpers::source(),
        OsStr::new("HEAD"),
    ))
    .expect("a bounded history remains a usable snapshot");
    assert_eq!(history.history().status(), EvidenceStatus::Partial);
    assert!(history.history().truncated());
    assert_eq!(history.parent_delta().changed_entries(), 2);
    assert_eq!(history.parent_delta().renames(), 1);

    let rename_limited = helpers::adapter(CollectionLimits {
        max_rename_candidates: 1,
        ..CollectionLimits::default()
    })
    .collect(SnapshotRequest::new(
        history_fixture.path(),
        helpers::source(),
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
    let object_limited = helpers::adapter(CollectionLimits {
        max_object_bytes: 4,
        ..CollectionLimits::default()
    })
    .collect(SnapshotRequest::new(
        object_fixture.path(),
        helpers::source(),
        OsStr::new("HEAD"),
    ))
    .expect("oversized blobs must produce partial facts");
    assert_eq!(object_limited.status(), EvidenceStatus::Partial);
    assert!(object_limited.entries().iter().all(|entry| {
        entry.content().status() == EvidenceStatus::Partial
            && entry.content().issue() == Some(ObjectIssue::SizeLimit)
            && entry.content().content_hash().is_none()
    }));

    let record_error = helpers::adapter(CollectionLimits {
        max_tree_entries: 1,
        ..CollectionLimits::default()
    })
    .collect(SnapshotRequest::new(
        object_fixture.path(),
        helpers::source(),
        OsStr::new("HEAD"),
    ))
    .expect_err("tree record overflow must fail closed");
    assert_eq!(record_error.stage(), CollectionStage::EnumerateTree);
    assert_eq!(record_error.kind(), CollectionErrorKind::RecordLimit);
}

#[test]
fn collects_a_merge_commit_against_its_first_parent() {
    let fixture = RepositoryFixture::build(RepositoryScenario::TypeScriptProject)
        .expect("the deterministic fixture must build");
    helpers::run_git(
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
    helpers::run_git(fixture.path(), &[OsStr::new("add"), OsStr::new("--all")]);
    helpers::run_git(
        fixture.path(),
        &[
            OsStr::new("commit"),
            OsStr::new("--quiet"),
            OsStr::new("-m"),
            OsStr::new("Add feature branch"),
        ],
    );
    helpers::run_git(
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
    helpers::run_git(fixture.path(), &[OsStr::new("add"), OsStr::new("--all")]);
    helpers::run_git(
        fixture.path(),
        &[
            OsStr::new("commit"),
            OsStr::new("--quiet"),
            OsStr::new("-m"),
            OsStr::new("Add main branch"),
        ],
    );
    helpers::run_git(
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

    let snapshot = helpers::adapter(helpers::default_limits())
        .collect(SnapshotRequest::new(
            fixture.path(),
            helpers::source(),
            OsStr::new("HEAD"),
        ))
        .expect("a merge commit must collect with bounded first-parent facts");
    assert_eq!(snapshot.status(), EvidenceStatus::Complete);
    assert_eq!(snapshot.history().reachable_commits(), 4);
    assert_eq!(snapshot.parent_delta().status(), EvidenceStatus::Complete);
    assert_eq!(snapshot.parent_delta().changed_entries(), 1);
}

#[test]
fn supports_sha256_repositories_with_executable_and_empty_blobs() {
    let temporary = tempfile::tempdir().expect("the SHA-256 repository parent must be creatable");
    let repository = temporary.path().join("sha256-repository");
    let status = Command::new(helpers::trusted_git())
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
    helpers::run_git(&repository, &[OsStr::new("add"), OsStr::new("--all")]);
    helpers::run_git(
        &repository,
        &[
            OsStr::new("update-index"),
            OsStr::new("--chmod=+x"),
            OsStr::new("--"),
            OsStr::new("script.sh"),
        ],
    );
    helpers::run_git(
        &repository,
        &[
            OsStr::new("commit"),
            OsStr::new("--quiet"),
            OsStr::new("-m"),
            OsStr::new("Add SHA-256 object fixtures"),
        ],
    );

    let snapshot = helpers::adapter(helpers::default_limits())
        .collect(SnapshotRequest::new(
            &repository,
            helpers::source(),
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
    assert_eq!(
        helpers::entry(&snapshot, b"script.sh").mode(),
        assay_git::EntryMode::Executable
    );
    let empty = helpers::entry(&snapshot, b"empty.bin");
    assert_eq!(empty.content().size(), Some(0));
    assert_eq!(
        empty.content().content_hash().unwrap().as_str(),
        "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}
