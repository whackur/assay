use std::{ffi::OsStr, path::PathBuf, str::FromStr};

use assay_classifier::{BuiltInPolicy, LinguistAttributeFacts};
use assay_domain::{ContentHash, RepositorySource};
use assay_git::{CollectionLimits, GitCliAdapter, RepositorySnapshotPort, SnapshotRequest};
use assay_project_intelligence::{
    ClassifiedSnapshotFile, assemble_project_evidence, build_project_analysis,
    validate_project_bundle_consistency,
};
use assay_test_fixtures::{RepositoryFixture, RepositoryScenario, trusted_git_executable};
use serde_json::Value;

use super::features::refresh_project_artifact;
use super::ids::repository_feature_id;

pub fn coherent_bundle() -> Value {
    serde_json::from_str(include_str!(
        "../../../../tests/golden/project-analysis-v1.json"
    ))
    .expect("reviewed project-analysis golden must parse")
}

pub fn bundle_with_present_feature() -> Value {
    let mut bundle = coherent_bundle();
    let tracked = tracked_file_record();
    let related = [tracked["id"].as_str().unwrap()];
    let id = repository_feature_id(&bundle, "readme", "present", &related);
    let feature = serde_json::json!({
        "schema_version": "1.0.0",
        "repository": bundle["manifest"]["source_snapshot"]["source"].clone(),
        "id": id,
        "status": "complete",
        "grade": "a",
        "privacy": {
            "visibility": "public",
            "source_content": "not_retained",
            "external_transmission": "not_requested"
        },
        "provenance": {
            "source_kind": "repository_content",
            "collected_at": "2026-01-02T03:04:06Z",
            "repository_revision": bundle["manifest"]["source_snapshot"]["revision"].clone(),
            "content_hash": null,
            "remote_record_id": null
        },
        "payload": {
            "kind": "repository_feature",
            "feature": "readme",
            "state": "present",
            "related_evidence_ids": related
        }
    });
    bundle["evidence"]
        .as_array_mut()
        .unwrap()
        .extend([tracked, feature]);
    refresh_project_artifact(&mut bundle);
    validate_project_bundle_consistency(&bundle).expect("baseline feature bundle must validate");
    bundle
}

pub fn real_producer_bundle() -> Value {
    let fixture = RepositoryFixture::build(RepositoryScenario::MissingReadmeAndLicense).unwrap();
    let source = RepositorySource::local(
        ContentHash::from_str(&format!("sha256:{}", "1".repeat(64))).unwrap(),
    );
    let snapshot =
        GitCliAdapter::from_trusted_executable(trusted_git(), CollectionLimits::default())
            .unwrap()
            .collect(SnapshotRequest::new(
                fixture.path(),
                source,
                OsStr::new("HEAD"),
            ))
            .unwrap();
    let classifications = snapshot
        .entries()
        .iter()
        .map(|entry| {
            ClassifiedSnapshotFile::classify(
                &snapshot,
                entry,
                LinguistAttributeFacts::available(None, None),
                &BuiltInPolicy::V1,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let manifest = assemble_project_evidence(&snapshot, classifications).unwrap();
    let bundle = build_project_analysis(&snapshot, &manifest, "2026-01-02T03:04:06Z").unwrap();
    validate_project_bundle_consistency(&bundle).expect("real producer bundle must validate");
    bundle
}

pub fn trusted_git() -> PathBuf {
    trusted_git_executable().expect("tests require a deployment-trusted Git executable")
}

pub fn tracked_file_record() -> Value {
    serde_json::json!({
        "schema_version": "1.0.0",
        "repository": {
            "kind": "local",
            "repository_id": format!("sha256:{}", "1".repeat(64))
        },
        "id": "evidence:tracked-file:v1-golden",
        "status": "complete",
        "grade": "a",
        "privacy": {
            "visibility": "public",
            "source_content": "not_retained",
            "external_transmission": "not_requested"
        },
        "provenance": {
            "source_kind": "repository_content",
            "collected_at": "2026-01-02T03:04:06Z",
            "repository_revision": "0123456789abcdef0123456789abcdef01234567",
            "content_hash": format!("sha256:{}", "4".repeat(64)),
            "remote_record_id": null
        },
        "payload": {
            "kind": "tracked_file",
            "path": { "encoding": "utf8", "value": "README.md" },
            "mode": "regular",
            "object_kind": "blob",
            "object_id": "89abcdef0123456789abcdef0123456789abcdef",
            "content_status": "complete",
            "language": "Markdown",
            "language_status": "complete",
            "size_bytes": 12,
            "content_hash": format!("sha256:{}", "4".repeat(64)),
            "issue": null
        }
    })
}
