#![cfg(unix)]

mod security_boundaries_helpers;

use assay_git::{CollectionLimits, GitCliAdapter, RepositorySnapshotPort};
use security_boundaries_helpers as helpers;
use serial_test::serial;
use std::{env, fs, os::unix::fs::PermissionsExt, process::Command};

const CHILD_MARKER: &str = "ASSAY_HOSTILE_ENV_CHILD";

#[test]
#[serial]
fn hostile_inherited_environment_and_repository_configuration_are_isolated() {
    if env::var_os(CHILD_MARKER).is_some() {
        hostile_environment_child();
        return;
    }

    let current_test = env::current_exe().expect("the integration test executable must exist");
    let trap_parent = tempfile::tempdir().expect("the hostile parent must be creatable");
    let status = Command::new(current_test)
        .arg("--exact")
        .arg("hostile_inherited_environment_and_repository_configuration_are_isolated")
        .arg("--nocapture")
        .env(CHILD_MARKER, "1")
        .env("GIT_DIR", trap_parent.path().join("redirected-git-dir"))
        .env(
            "GIT_OBJECT_DIRECTORY",
            trap_parent.path().join("redirected-objects"),
        )
        .env(
            "GIT_INDEX_FILE",
            trap_parent.path().join("redirected-index"),
        )
        .env(
            "GIT_WORK_TREE",
            trap_parent.path().join("redirected-worktree"),
        )
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "core.pager")
        .env("GIT_CONFIG_VALUE_0", "hostile-pager")
        .env("GIT_ASKPASS", "hostile-askpass")
        .env("GIT_SSH_COMMAND", "hostile-ssh")
        .env("GIT_TRACE", "1")
        .env("GIT_TRACE2", "1")
        .status()
        .expect("the isolated hostile-environment child must start");
    assert!(status.success());
}

fn hostile_environment_child() {
    let trap_parent = tempfile::tempdir().expect("the trap parent must be creatable");
    let trap = trap_parent.path().join("repository-command-invoked");
    let fixture = helpers::fixture();
    helpers::git_config(fixture.path(), "core.pager", trap.as_os_str());
    helpers::git_config(
        fixture.path(),
        "core.hooksPath",
        trap_parent.path().as_os_str(),
    );
    helpers::git_config(fixture.path(), "diff.external", trap.as_os_str());
    helpers::git_config(fixture.path(), "diff.trap.command", trap.as_os_str());
    helpers::git_config(fixture.path(), "filter.trap.clean", trap.as_os_str());
    helpers::git_config(fixture.path(), "filter.trap.smudge", trap.as_os_str());
    helpers::git_config(fixture.path(), "credential.helper", trap.as_os_str());

    let wrapper_script = r#"
[ -z "${GIT_DIR+x}" ] || exit 71
[ -z "${GIT_OBJECT_DIRECTORY+x}" ] || exit 72
[ -z "${GIT_INDEX_FILE+x}" ] || exit 73
[ -z "${GIT_WORK_TREE+x}" ] || exit 74
[ -z "${GIT_CONFIG_COUNT+x}" ] || exit 75
[ -z "${GIT_ASKPASS+x}" ] || exit 76
[ -z "${GIT_SSH_COMMAND+x}" ] || exit 77
[ "${GIT_NO_LAZY_FETCH-}" = "1" ] || exit 78
[ "${GIT_NO_REPLACE_OBJECTS-}" = "1" ] || exit 79
[ "${GIT_OPTIONAL_LOCKS-}" = "0" ] || exit 80
[ "${GIT_TERMINAL_PROMPT-}" = "0" ] || exit 81
[ "${GIT_TRACE-}" = "0" ] || exit 82
[ "${GIT_TRACE2-}" = "0" ] || exit 83
found_lazy_fetch=0
for argument in "$@"; do
  [ "$argument" = "--no-lazy-fetch" ] && found_lazy_fetch=1
done
[ "$found_lazy_fetch" = "1" ] || exit 84
exec /usr/bin/git "$@"
"#;
    let (_directory, checked_git) = helpers::wrapper(wrapper_script);
    let adapter = GitCliAdapter::from_trusted_executable(checked_git, CollectionLimits::default())
        .expect("the sanitized wrapper must pass the capability probe");
    adapter
        .collect(helpers::request(fixture.path()))
        .expect("hostile host and repository configuration must be inert");

    assert!(!trap.exists());
}

#[test]
#[serial]
fn missing_promisor_object_never_invokes_transport_or_helper_or_mutates_objects() {
    let fixture = helpers::fixture();
    let trap_parent = tempfile::tempdir().expect("the transport trap parent must be creatable");
    let transport_trap = trap_parent.path().join("transport-invoked");
    let credential_trap = trap_parent.path().join("credential-invoked");
    let transport_script = trap_parent.path().join("transport-helper");
    fs::write(
        &transport_script,
        format!("#!/bin/sh\ntouch '{}'\nexit 91\n", transport_trap.display()),
    )
    .expect("the transport trap must be writable");
    fs::set_permissions(&transport_script, fs::Permissions::from_mode(0o755))
        .expect("the transport trap must be executable");

    helpers::git_config(
        fixture.path(),
        "core.repositoryformatversion",
        std::ffi::OsStr::new("1"),
    );
    helpers::git_config(
        fixture.path(),
        "extensions.partialclone",
        std::ffi::OsStr::new("origin"),
    );
    helpers::git_config(
        fixture.path(),
        "remote.origin.promisor",
        std::ffi::OsStr::new("true"),
    );
    helpers::git_config(
        fixture.path(),
        "remote.origin.partialclonefilter",
        std::ffi::OsStr::new("blob:none"),
    );
    let remote = format!("ext::{}", transport_script.display());
    helpers::git_config(
        fixture.path(),
        "remote.origin.url",
        std::ffi::OsStr::new(&remote),
    );
    helpers::git_config(
        fixture.path(),
        "protocol.ext.allow",
        std::ffi::OsStr::new("always"),
    );
    let credential = format!("!touch '{}'", credential_trap.display());
    helpers::git_config(
        fixture.path(),
        "credential.helper",
        std::ffi::OsStr::new(&credential),
    );

    let missing_object = helpers::git_stdout(
        fixture.path(),
        &[
            std::ffi::OsStr::new("rev-parse"),
            std::ffi::OsStr::new("HEAD:src/add.ts"),
        ],
    );
    let missing_object =
        std::str::from_utf8(missing_object.trim_ascii_end()).expect("the blob ID must be ASCII");
    let loose_object = fixture
        .path()
        .join(".git/objects")
        .join(&missing_object[..2])
        .join(&missing_object[2..]);
    assert!(loose_object.is_file());
    fs::remove_file(&loose_object).expect("the promised blob must be removed synthetically");
    let before = helpers::object_store_fingerprint(fixture.path());

    let snapshot = helpers::adapter(CollectionLimits::default())
        .collect(helpers::request(fixture.path()))
        .expect("a missing promised blob must remain a partial snapshot");
    let missing = snapshot
        .entries()
        .iter()
        .find(|entry| entry.path().as_bytes() == b"src/add.ts")
        .expect("the missing promised entry must remain listed");

    assert_eq!(snapshot.status(), assay_domain::EvidenceStatus::Partial);
    assert_eq!(
        missing.content().status(),
        assay_domain::EvidenceStatus::Unavailable
    );
    assert_eq!(
        missing.content().issue(),
        Some(assay_git::ObjectIssue::MissingOrUnreadable)
    );
    assert!(!transport_trap.exists());
    assert!(!credential_trap.exists());
    assert_eq!(before, helpers::object_store_fingerprint(fixture.path()));
}
