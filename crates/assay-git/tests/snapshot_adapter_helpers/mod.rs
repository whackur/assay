use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    process::Command,
    str::FromStr,
    time::Duration,
};

use assay_domain::{ContentHash, RepositorySource};
use assay_git::{
    CollectionLimits, GitCliAdapter, ParentDeltaIssue, RepositorySnapshot, TrackedEntry,
};
use assay_test_fixtures::trusted_git_executable;

pub fn trusted_git() -> PathBuf {
    trusted_git_executable()
        .expect("the Git adapter integration tests require a trusted absolute Git executable")
}

pub fn source() -> RepositorySource {
    RepositorySource::local(
        ContentHash::from_str(
            "sha256:1111111111111111111111111111111111111111111111111111111111111111",
        )
        .expect("the test repository ID must be valid"),
    )
}

pub fn adapter(limits: CollectionLimits) -> GitCliAdapter {
    GitCliAdapter::from_trusted_executable(trusted_git(), limits)
        .expect("the installed Git must satisfy the recorded baseline")
}

pub fn default_limits() -> CollectionLimits {
    CollectionLimits::default()
}

#[allow(dead_code)]
pub fn entry<'a>(snapshot: &'a RepositorySnapshot, path: &[u8]) -> &'a TrackedEntry {
    snapshot
        .entries()
        .iter()
        .find(|entry| entry.path().as_bytes() == path)
        .expect("the expected tracked entry must exist")
}

#[allow(dead_code)]
pub fn run_git(repository: &Path, arguments: &[&OsStr]) {
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
pub fn short_timeout_limits() -> CollectionLimits {
    CollectionLimits {
        command_timeout: Duration::from_millis(100),
        ..CollectionLimits::default()
    }
}

#[allow(dead_code)]
pub fn parent_limit_issue_is_public() -> ParentDeltaIssue {
    ParentDeltaIssue::RenameCandidateLimit
}
