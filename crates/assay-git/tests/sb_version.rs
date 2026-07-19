#![cfg(unix)]

mod security_boundaries_helpers;

use assay_git::{
    CollectionErrorKind, CollectionLimits, CollectionStage, GitCliAdapter, RepositorySnapshotPort,
};
use security_boundaries_helpers as helpers;
use serial_test::serial;

#[test]
#[serial]
fn rejects_incompatible_git_and_malformed_nul_protocol() {
    let (_directory, old_git) = helpers::wrapper("printf 'git version 2.46.0\\n'");
    let old = GitCliAdapter::from_trusted_executable(old_git, CollectionLimits::default())
        .expect_err("Git older than the ADR baseline must fail closed");
    assert_eq!(old.stage(), CollectionStage::ProbeCapabilities);
    assert_eq!(old.kind(), CollectionErrorKind::IncompatibleGit);

    let no_capability = r#"
for argument in "$@"; do
  [ "$argument" = "--no-lazy-fetch" ] && exit 129
done
exec /usr/bin/git "$@"
"#;
    let (_directory, no_capability_git) = helpers::wrapper(no_capability);
    let unsupported =
        GitCliAdapter::from_trusted_executable(no_capability_git, CollectionLimits::default())
            .expect_err("an executable without --no-lazy-fetch must fail closed");
    assert_eq!(unsupported.stage(), CollectionStage::ProbeCapabilities);
    assert_eq!(unsupported.kind(), CollectionErrorKind::IncompatibleGit);

    let script = r#"
for argument in "$@"; do
  if [ "$argument" = "ls-tree" ]; then
    printf '100644 blob 1111111111111111111111111111111111111111\tunterminated'
    exit 0
  fi
done
exec /usr/bin/git "$@"
"#;
    let (_directory, malformed_git) = helpers::wrapper(script);
    let fixture = helpers::fixture();
    let adapter =
        GitCliAdapter::from_trusted_executable(malformed_git, CollectionLimits::default())
            .expect("the wrapper must pass the capability probe");
    let error = adapter
        .collect(helpers::request(fixture.path()))
        .expect_err("unterminated NUL protocol must fail closed");
    assert_eq!(error.stage(), CollectionStage::EnumerateTree);
    assert_eq!(error.kind(), CollectionErrorKind::MalformedOutput);
}

#[test]
#[serial]
fn accepts_a_bounded_vendor_git_version_suffix() {
    let (_directory, apple_git) =
        helpers::wrapper("printf 'git version 2.47.3 (Apple Git-145)\\n'");
    GitCliAdapter::from_trusted_executable(apple_git, CollectionLimits::default())
        .expect("a bounded visible vendor suffix must be accepted");

    let (_directory, windows_git) = helpers::wrapper("printf 'git version 2.47.3.windows.1\\n'");
    GitCliAdapter::from_trusted_executable(windows_git, CollectionLimits::default())
        .expect("a bounded Git for Windows suffix must be accepted");

    let (_directory, control_git) =
        helpers::wrapper("printf 'git version 2.47.3 (bad\\tvendor)\\n'");
    let error = GitCliAdapter::from_trusted_executable(control_git, CollectionLimits::default())
        .expect_err("control characters in a version must fail closed");
    assert_eq!(error.kind(), CollectionErrorKind::IncompatibleGit);
}

#[test]
#[serial]
fn rejects_malformed_or_multiline_commit_time() {
    for reported in ["not-a-time\\n", "2001-02-03T04:05:06+09:00\\nextra\\n"] {
        let body = format!(
            r#"
for argument in "$@"; do
  if [ "$argument" = "--format=%cI" ]; then
    printf '{}'
    exit 0
  fi
done
exec /usr/bin/git "$@"
"#,
            reported
        );
        let (_directory, executable) = helpers::wrapper(&body);
        let error = GitCliAdapter::from_trusted_executable(executable, CollectionLimits::default())
            .expect("the wrapper must pass the capability probe")
            .collect(helpers::request(helpers::fixture().path()))
            .expect_err("invalid commit time must fail closed");
        assert_eq!(error.stage(), CollectionStage::ReadCommitTime);
        assert_eq!(error.kind(), CollectionErrorKind::MalformedOutput);
    }
}
