use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    str::FromStr,
};

use assay_classifier::{BuiltInPolicy, LinguistAttributeFacts};
use assay_domain::{ContentHash, RepositorySource};
use assay_git::{
    CollectionLimits, GitCliAdapter, RepositorySnapshot, RepositorySnapshotPort, SnapshotRequest,
};
use assay_project_intelligence::ClassifiedSnapshotFile;
use assay_test_fixtures::{RepositoryFixture, RepositoryScenario, trusted_git_executable};

pub fn trusted_git() -> PathBuf {
    trusted_git_executable().expect("tests require a deployment-trusted Git executable")
}

pub fn source_with_digest(digit: char) -> RepositorySource {
    RepositorySource::local(
        ContentHash::from_str(&format!("sha256:{}", digit.to_string().repeat(64))).unwrap(),
    )
}

pub fn source() -> RepositorySource {
    source_with_digest('1')
}

pub fn snapshot(scenario: RepositoryScenario, limits: CollectionLimits) -> RepositorySnapshot {
    let fixture = RepositoryFixture::build(scenario).expect("fixture must build");
    collect_snapshot(fixture.path(), OsStr::new("HEAD"), source(), limits)
}

pub fn collect_snapshot(
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

pub fn classifications(
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

pub fn related_ids(feature: &serde_json::Value) -> Vec<String> {
    feature["payload"]["related_evidence_ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|id| id.as_str().unwrap().to_owned())
        .collect()
}

pub fn feature<'a>(output: &'a serde_json::Value, name: &str) -> &'a serde_json::Value {
    output["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .find(|fact| fact["payload"]["feature"] == name)
        .unwrap()
}
