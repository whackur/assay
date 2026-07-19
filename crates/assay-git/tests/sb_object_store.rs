#![cfg(unix)]

mod security_boundaries_helpers;

use assay_git::{
    CollectionErrorKind, CollectionLimits, CollectionStage, GitCliAdapter, RepositorySnapshotPort,
};
use security_boundaries_helpers as helpers;
use serial_test::serial;
use std::{ffi::OsStr, fs};

#[test]
#[serial]
fn alternate_and_symlinked_object_stores_fail_before_object_access() {
    let fixture = helpers::fixture();
    let external = tempfile::tempdir().expect("the external object store must be creatable");
    fs::write(
        fixture.path().join(".git/objects/info/alternates"),
        external.path().as_os_str().as_encoded_bytes(),
    )
    .expect("the alternate object path must be writable");
    let error = helpers::adapter(CollectionLimits::default())
        .collect(helpers::request(fixture.path()))
        .expect_err("alternate object stores must be rejected");
    assert_eq!(error.stage(), CollectionStage::ValidateObjectStore);
    assert_eq!(error.kind(), CollectionErrorKind::ExternalObjectStore);
    assert!(!format!("{error:?}").contains(external.path().to_string_lossy().as_ref()));

    fs::remove_file(fixture.path().join(".git/objects/info/alternates"))
        .expect("the synthetic alternate file must be removable");
    let object_id = helpers::git_stdout(
        fixture.path(),
        &[OsStr::new("rev-parse"), OsStr::new("HEAD:src/add.ts")],
    );
    let object_id =
        std::str::from_utf8(object_id.trim_ascii_end()).expect("the object ID must be ASCII");
    let object = fixture
        .path()
        .join(".git/objects")
        .join(&object_id[..2])
        .join(&object_id[2..]);
    let original = object.with_extension("original");
    fs::rename(&object, &original).expect("the loose object must be movable");
    std::os::unix::fs::symlink(&original, &object)
        .expect("the synthetic object symlink must be creatable");
    let error = helpers::adapter(CollectionLimits::default())
        .collect(helpers::request(fixture.path()))
        .expect_err("symlinked object entries must be rejected");
    assert_eq!(error.stage(), CollectionStage::ValidateObjectStore);
    assert_eq!(error.kind(), CollectionErrorKind::ExternalObjectStore);
}

#[test]
#[serial]
fn nonzero_child_exit_is_redacted() {
    let script = r#"
for argument in "$@"; do
  if [ "$argument" = "rev-parse" ]; then
    printf 'secret source text and /machine/path\n' >&2
    exit 23
  fi
done
exec /usr/bin/git "$@"
"#;
    let (_directory, failing_git) = helpers::wrapper(script);
    let fixture = helpers::fixture();
    let adapter = GitCliAdapter::from_trusted_executable(failing_git, CollectionLimits::default())
        .expect("the failure wrapper must pass the capability probe");
    let error = adapter
        .collect(helpers::request(fixture.path()))
        .expect_err("the synthetic command must fail");
    let debug = format!("{error:?}");
    let display = format!("{error}");

    assert_eq!(error.stage(), CollectionStage::ValidateObjectStore);
    assert_eq!(error.kind(), CollectionErrorKind::NonZeroExit);
    assert!(!debug.contains("secret"));
    assert!(!display.contains("machine"));
    assert!(!display.contains("23"));
}
