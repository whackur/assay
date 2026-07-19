mod snapshot_adapter_helpers;

use assay_git::{RepositorySnapshotPort, SnapshotRequest};
use assay_test_fixtures::{RepositoryFixture, RepositoryScenario};
use snapshot_adapter_helpers as helpers;
use std::{ffi::OsStr, process::Command};

#[test]
fn local_repository_identity_is_stable_across_host_paths() {
    let first = RepositoryFixture::build(RepositoryScenario::TypeScriptProject)
        .expect("the first deterministic fixture must build");
    let second = RepositoryFixture::build(RepositoryScenario::TypeScriptProject)
        .expect("the second deterministic fixture must build");
    assert_ne!(first.path(), second.path());

    let adapter = helpers::adapter(helpers::default_limits());
    let first_source = adapter
        .derive_local_repository_source(first.path(), OsStr::new("HEAD"))
        .expect("the first identity must derive");
    let second_source = adapter
        .derive_local_repository_source(second.path(), OsStr::new("HEAD"))
        .expect("the second identity must derive");

    assert_eq!(first_source.source(), second_source.source());
    assert_eq!(first_source.revision(), second_source.revision());
    assert!(first_source.source().local_repository_id().is_some());
}

#[test]
fn resolved_local_identity_pins_collection_across_a_head_move() {
    let fixture = RepositoryFixture::build(RepositoryScenario::DependencyOnlyChange)
        .expect("the deterministic fixture must build");
    let adapter = helpers::adapter(helpers::default_limits());
    let identity = adapter
        .derive_local_repository_source(fixture.path(), OsStr::new("HEAD"))
        .expect("identity and exact revision must derive together");
    let expected = identity.revision().as_str().to_owned();
    let first = fixture.commit_ids().first().unwrap();
    let status = Command::new(helpers::trusted_git())
        .current_dir(fixture.path())
        .args(["reset", "--hard", "--quiet", first])
        .status()
        .expect("HEAD move must execute");
    assert!(status.success());

    let snapshot = adapter
        .collect(SnapshotRequest::new(
            fixture.path(),
            identity.source().clone(),
            OsStr::new(&expected),
        ))
        .expect("the immutable resolved commit must remain collectable");
    assert_eq!(snapshot.source_snapshot().revision().as_str(), expected);
}

#[test]
fn treats_a_revision_beginning_with_dash_as_an_operand() {
    let fixture = RepositoryFixture::build(RepositoryScenario::TypeScriptProject)
        .expect("the deterministic fixture must build");
    let error = helpers::adapter(helpers::default_limits())
        .collect(SnapshotRequest::new(
            fixture.path(),
            helpers::source(),
            OsStr::new("--help"),
        ))
        .expect_err("an option-shaped revision must not become an option");

    assert_eq!(error.stage(), assay_git::CollectionStage::ResolveRevision);
    assert_eq!(error.kind(), assay_git::CollectionErrorKind::NonZeroExit);
    assert!(!format!("{error:?}").contains(fixture.path().to_string_lossy().as_ref()));
    assert!(!format!("{error}").contains("--help"));
}
