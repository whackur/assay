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
fn rejects_empty_records_in_reachable_root_identity() {
    let script = r#"
for argument in "$@"; do
  if [ "$argument" = "rev-list" ]; then
    printf '1111111111111111111111111111111111111111\n\n2222222222222222222222222222222222222222\n'
    exit 0
  fi
done
exec /usr/bin/git "$@"
"#;
    let (_directory, executable) = helpers::wrapper(script);
    let fixture = helpers::fixture();
    let error = GitCliAdapter::from_trusted_executable(executable, CollectionLimits::default())
        .expect("the wrapper must pass the capability probe")
        .derive_local_repository_source(fixture.path(), OsStr::new("HEAD"))
        .expect_err("an empty root record must fail closed");
    assert_eq!(error.stage(), CollectionStage::DeriveRepositoryIdentity);
    assert_eq!(error.kind(), CollectionErrorKind::MalformedOutput);
}

#[test]
#[serial]
fn rejects_symlinked_dot_git_and_unrelated_gitdir_redirects() {
    let target = helpers::fixture();
    let parent = tempfile::tempdir().expect("the submitted parent must be creatable");
    let symlinked = parent.path().join("symlinked");
    fs::create_dir(&symlinked).expect("the submitted directory must be creatable");
    std::os::unix::fs::symlink(target.path().join(".git"), symlinked.join(".git"))
        .expect("the malicious .git symlink must be creatable");
    let error = helpers::adapter(CollectionLimits::default())
        .collect(helpers::request(&symlinked))
        .expect_err("a .git symlink must fail before Git execution");
    assert_eq!(error.stage(), CollectionStage::ValidateObjectStore);
    assert_eq!(error.kind(), CollectionErrorKind::RepositoryRedirect);

    let redirected = parent.path().join("redirected");
    fs::create_dir(&redirected).expect("the redirected directory must be creatable");
    fs::write(
        redirected.join(".git"),
        format!("gitdir: {}\n", target.path().join(".git").display()),
    )
    .expect("the malicious gitdir pointer must be writable");
    let error = helpers::adapter(CollectionLimits::default())
        .collect(helpers::request(&redirected))
        .expect_err("an unrelated gitdir pointer must fail backlink validation");
    assert_eq!(error.stage(), CollectionStage::ValidateObjectStore);
    assert_eq!(error.kind(), CollectionErrorKind::RepositoryRedirect);
    assert!(!format!("{error:?}").contains(target.path().to_string_lossy().as_ref()));

    let linked = target
        .path()
        .parent()
        .expect("the fixture has a temporary parent")
        .join("linked-for-redirect");
    let status = helpers::synthetic_git(target.path())
        .args(["worktree", "add", "--quiet", "--detach"])
        .arg(&linked)
        .arg("HEAD")
        .status()
        .expect("the synthetic linked-worktree command must start");
    assert!(status.success());
    let dot_git = fs::read_to_string(linked.join(".git"))
        .expect("the linked-worktree pointer must be readable");
    let admin = dot_git
        .trim()
        .strip_prefix("gitdir: ")
        .expect("Git must write a gitdir pointer");
    let admin_link = parent.path().join("admin-link");
    std::os::unix::fs::symlink(admin, &admin_link)
        .expect("the synthetic admin-directory link must be creatable");
    fs::write(
        linked.join(".git"),
        format!("gitdir: {}\n", admin_link.display()),
    )
    .expect("the linked-worktree pointer must be replaceable");
    let error = helpers::adapter(CollectionLimits::default())
        .collect(helpers::request(&linked))
        .expect_err("a final gitdir symlink must fail before canonicalization");
    assert_eq!(error.stage(), CollectionStage::ValidateObjectStore);
    assert_eq!(error.kind(), CollectionErrorKind::RepositoryRedirect);
}
