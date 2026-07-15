#![cfg(unix)]

use std::{
    env,
    ffi::OsStr,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
    str::FromStr,
    time::{Duration, Instant},
};

use assay_domain::{ContentHash, EvidenceStatus, RepositorySource};
use assay_git::{
    CollectionErrorKind, CollectionLimits, CollectionStage, GitCliAdapter, ObjectIssue,
    ParentDeltaIssue, RepositorySnapshotPort, SnapshotRequest,
};
use assay_test_fixtures::{RepositoryFixture, RepositoryScenario};
use tempfile::TempDir;

const CHILD_MARKER: &str = "ASSAY_HOSTILE_ENV_CHILD";

#[test]
fn rejects_incompatible_git_and_malformed_nul_protocol() {
    let (_directory, old_git) = wrapper("printf 'git version 2.46.0\\n'");
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
    let (_directory, no_capability_git) = wrapper(no_capability);
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
    let (_directory, malformed_git) = wrapper(script);
    let fixture = fixture();
    let adapter =
        GitCliAdapter::from_trusted_executable(malformed_git, CollectionLimits::default())
            .expect("the wrapper must pass the capability probe");
    let error = adapter
        .collect(request(fixture.path()))
        .expect_err("unterminated NUL protocol must fail closed");
    assert_eq!(error.stage(), CollectionStage::EnumerateTree);
    assert_eq!(error.kind(), CollectionErrorKind::MalformedOutput);
}

#[test]
fn accepts_a_bounded_vendor_git_version_suffix() {
    let (_directory, apple_git) = wrapper("printf 'git version 2.47.3 (Apple Git-145)\\n'");
    GitCliAdapter::from_trusted_executable(apple_git, CollectionLimits::default())
        .expect("a bounded visible vendor suffix must be accepted");

    let (_directory, windows_git) = wrapper("printf 'git version 2.47.3.windows.1\\n'");
    GitCliAdapter::from_trusted_executable(windows_git, CollectionLimits::default())
        .expect("a bounded Git for Windows suffix must be accepted");

    let (_directory, control_git) = wrapper("printf 'git version 2.47.3 (bad\\tvendor)\\n'");
    let error = GitCliAdapter::from_trusted_executable(control_git, CollectionLimits::default())
        .expect_err("control characters in a version must fail closed");
    assert_eq!(error.kind(), CollectionErrorKind::IncompatibleGit);
}

#[test]
fn rejects_symlinked_dot_git_and_unrelated_gitdir_redirects() {
    let target = fixture();
    let parent = tempfile::tempdir().expect("the submitted parent must be creatable");
    let symlinked = parent.path().join("symlinked");
    fs::create_dir(&symlinked).expect("the submitted directory must be creatable");
    std::os::unix::fs::symlink(target.path().join(".git"), symlinked.join(".git"))
        .expect("the malicious .git symlink must be creatable");
    let error = adapter(CollectionLimits::default())
        .collect(request(&symlinked))
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
    let error = adapter(CollectionLimits::default())
        .collect(request(&redirected))
        .expect_err("an unrelated gitdir pointer must fail backlink validation");
    assert_eq!(error.stage(), CollectionStage::ValidateObjectStore);
    assert_eq!(error.kind(), CollectionErrorKind::RepositoryRedirect);
    assert!(!format!("{error:?}").contains(target.path().to_string_lossy().as_ref()));

    let linked = target
        .path()
        .parent()
        .expect("the fixture has a temporary parent")
        .join("linked-for-redirect");
    let status = synthetic_git(target.path())
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
    let error = adapter(CollectionLimits::default())
        .collect(request(&linked))
        .expect_err("a final gitdir symlink must fail before canonicalization");
    assert_eq!(error.stage(), CollectionStage::ValidateObjectStore);
    assert_eq!(error.kind(), CollectionErrorKind::RepositoryRedirect);
}

#[test]
fn one_deadline_kills_an_exited_parents_pipe_holding_grandchild() {
    let wrapper_parent = tempfile::tempdir().expect("the wrapper parent must be creatable");
    let pid_file = wrapper_parent.path().join("grandchild.pid");
    let body = format!(
        r#"
for argument in "$@"; do
  if [ "$argument" = "rev-parse" ]; then
    /bin/sleep 5 &
    printf '%s\n' "$!" > '{}'
    exit 0
  fi
done
exec /usr/bin/git "$@"
"#,
        pid_file.display()
    );
    let executable = wrapper_parent.path().join("git-wrapper");
    write_wrapper(&executable, &body);
    let limits = CollectionLimits {
        command_timeout: Duration::from_millis(100),
        ..CollectionLimits::default()
    };
    let adapter = GitCliAdapter::from_trusted_executable(executable, limits)
        .expect("the orphan wrapper must pass the capability probe");
    let fixture = fixture();
    let started = Instant::now();
    let error = adapter
        .collect(request(fixture.path()))
        .expect_err("a pipe-holding grandchild must hit the complete command deadline");
    assert_eq!(error.kind(), CollectionErrorKind::Timeout);
    assert!(started.elapsed() < Duration::from_secs(2));

    let pid = fs::read_to_string(pid_file)
        .expect("the synthetic grandchild PID must be recorded")
        .trim()
        .to_owned();
    let process = PathBuf::from("/proc").join(pid);
    for _ in 0..50 {
        if !process.exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        !process.exists(),
        "the killed process group must leave no child"
    );
}

#[test]
fn rejects_mixed_object_ids_and_impossible_raw_statuses() {
    let mixed_tree = r#"
for argument in "$@"; do
  if [ "$argument" = "ls-tree" ]; then
    printf '100644 blob 1111111111111111111111111111111111111111111111111111111111111111\tfile\000'
    exit 0
  fi
done
exec /usr/bin/git "$@"
"#;
    let (_directory, mixed_git) = wrapper(mixed_tree);
    let fixture = fixture();
    let adapter = GitCliAdapter::from_trusted_executable(mixed_git, CollectionLimits::default())
        .expect("the mixed-ID wrapper must pass the capability probe");
    let error = adapter
        .collect(request(fixture.path()))
        .expect_err("a SHA-256 tree ID inside a SHA-1 repository must fail closed");
    assert_eq!(error.stage(), CollectionStage::EnumerateTree);
    assert_eq!(error.kind(), CollectionErrorKind::MalformedOutput);

    let impossible_add = r#"
for argument in "$@"; do
  if [ "$argument" = "diff-tree" ]; then
    printf ':100644 100644 1111111111111111111111111111111111111111 2222222222222222222222222222222222222222 A\000path\000'
    exit 0
  fi
done
exec /usr/bin/git "$@"
"#;
    let (_directory, impossible_git) = wrapper(impossible_add);
    let fixture = RepositoryFixture::build(RepositoryScenario::RenameAndMove)
        .expect("the rename fixture must build");
    let adapter =
        GitCliAdapter::from_trusted_executable(impossible_git, CollectionLimits::default())
            .expect("the impossible-status wrapper must pass the capability probe");
    let snapshot = adapter
        .collect(request(fixture.path()))
        .expect("invalid optional delta evidence must leave a partial snapshot");
    assert_eq!(
        snapshot.parent_delta().status(),
        EvidenceStatus::Unavailable
    );
    assert_eq!(
        snapshot.parent_delta().issue(),
        Some(ParentDeltaIssue::MalformedOutput)
    );
}

#[test]
fn bounds_timeout_and_concurrently_drains_both_pipes() {
    let timeout_script = r#"
for argument in "$@"; do
  if [ "$argument" = "rev-parse" ]; then
    /bin/sleep 5 &
    exec /bin/sleep 5
  fi
done
exec /usr/bin/git "$@"
"#;
    let (_directory, timeout_git) = wrapper(timeout_script);
    let limits = CollectionLimits {
        command_timeout: Duration::from_millis(100),
        ..CollectionLimits::default()
    };
    let adapter = GitCliAdapter::from_trusted_executable(timeout_git, limits)
        .expect("the timeout wrapper must pass the capability probe");
    let fixture = fixture();
    let started = Instant::now();
    let error = adapter
        .collect(request(fixture.path()))
        .expect_err("a hung Git command must time out");
    assert_eq!(error.stage(), CollectionStage::ValidateObjectStore);
    assert_eq!(error.kind(), CollectionErrorKind::Timeout);
    assert!(started.elapsed() < Duration::from_secs(2));

    let pipe_script = r#"
for argument in "$@"; do
  if [ "$argument" = "rev-parse" ]; then
    /usr/bin/head -c 131072 /dev/zero
    /usr/bin/head -c 131072 /dev/zero >&2
    exit 0
  fi
done
exec /usr/bin/git "$@"
"#;
    let (_directory, pipe_git) = wrapper(pipe_script);
    let limits = CollectionLimits {
        command_timeout: Duration::from_secs(2),
        max_stdout_bytes: 128,
        max_stderr_bytes: 128,
        ..CollectionLimits::default()
    };
    let adapter = GitCliAdapter::from_trusted_executable(pipe_git, limits)
        .expect("the pipe wrapper must pass the capability probe");
    let started = Instant::now();
    let error = adapter
        .collect(request(fixture.path()))
        .expect_err("oversized stdout and stderr must fail without deadlock");
    assert_eq!(error.stage(), CollectionStage::ValidateObjectStore);
    assert_eq!(error.kind(), CollectionErrorKind::OutputLimit);
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn malformed_raw_diff_is_an_explicit_unavailable_parent_delta() {
    let script = r#"
for argument in "$@"; do
  if [ "$argument" = "diff-tree" ]; then
    printf ':bogus header R100\000old\000new\000'
    exit 0
  fi
done
exec /usr/bin/git "$@"
"#;
    let (_directory, malformed_git) = wrapper(script);
    let fixture = RepositoryFixture::build(RepositoryScenario::RenameAndMove)
        .expect("the rename fixture must build");
    let adapter =
        GitCliAdapter::from_trusted_executable(malformed_git, CollectionLimits::default())
            .expect("the malformed diff wrapper must pass the capability probe");
    let snapshot = adapter
        .collect(request(fixture.path()))
        .expect("malformed optional delta evidence must preserve the source snapshot");

    assert_eq!(snapshot.status(), EvidenceStatus::Partial);
    assert_eq!(
        snapshot.parent_delta().status(),
        EvidenceStatus::Unavailable
    );
    assert_eq!(
        snapshot.parent_delta().issue(),
        Some(assay_git::ParentDeltaIssue::MalformedOutput)
    );
}

#[test]
fn alternate_and_symlinked_object_stores_fail_before_object_access() {
    let fixture = fixture();
    let external = tempfile::tempdir().expect("the external object store must be creatable");
    fs::write(
        fixture.path().join(".git/objects/info/alternates"),
        external.path().as_os_str().as_encoded_bytes(),
    )
    .expect("the alternate object path must be writable");
    let error = adapter(CollectionLimits::default())
        .collect(request(fixture.path()))
        .expect_err("alternate object stores must be rejected");
    assert_eq!(error.stage(), CollectionStage::ValidateObjectStore);
    assert_eq!(error.kind(), CollectionErrorKind::ExternalObjectStore);
    assert!(!format!("{error:?}").contains(external.path().to_string_lossy().as_ref()));

    fs::remove_file(fixture.path().join(".git/objects/info/alternates"))
        .expect("the synthetic alternate file must be removable");
    let object_id = git_stdout(
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
    let error = adapter(CollectionLimits::default())
        .collect(request(fixture.path()))
        .expect_err("symlinked object entries must be rejected");
    assert_eq!(error.stage(), CollectionStage::ValidateObjectStore);
    assert_eq!(error.kind(), CollectionErrorKind::ExternalObjectStore);
}

#[test]
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
    let (_directory, failing_git) = wrapper(script);
    let fixture = fixture();
    let adapter = GitCliAdapter::from_trusted_executable(failing_git, CollectionLimits::default())
        .expect("the failure wrapper must pass the capability probe");
    let error = adapter
        .collect(request(fixture.path()))
        .expect_err("the synthetic command must fail");
    let debug = format!("{error:?}");
    let display = format!("{error}");

    assert_eq!(error.stage(), CollectionStage::ValidateObjectStore);
    assert_eq!(error.kind(), CollectionErrorKind::NonZeroExit);
    assert!(!debug.contains("secret"));
    assert!(!display.contains("machine"));
    assert!(!display.contains("23"));
}

#[test]
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
    let fixture = fixture();
    git_config(fixture.path(), "core.pager", trap.as_os_str());
    git_config(
        fixture.path(),
        "core.hooksPath",
        trap_parent.path().as_os_str(),
    );
    git_config(fixture.path(), "diff.external", trap.as_os_str());
    git_config(fixture.path(), "diff.trap.command", trap.as_os_str());
    git_config(fixture.path(), "filter.trap.clean", trap.as_os_str());
    git_config(fixture.path(), "filter.trap.smudge", trap.as_os_str());
    git_config(fixture.path(), "credential.helper", trap.as_os_str());

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
    let (_directory, checked_git) = wrapper(wrapper_script);
    let adapter = GitCliAdapter::from_trusted_executable(checked_git, CollectionLimits::default())
        .expect("the sanitized wrapper must pass the capability probe");
    adapter
        .collect(request(fixture.path()))
        .expect("hostile host and repository configuration must be inert");

    assert!(!trap.exists());
}

#[test]
fn missing_promisor_object_never_invokes_transport_or_helper_or_mutates_objects() {
    let fixture = fixture();
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

    git_config(
        fixture.path(),
        "core.repositoryformatversion",
        OsStr::new("1"),
    );
    git_config(
        fixture.path(),
        "extensions.partialclone",
        OsStr::new("origin"),
    );
    git_config(fixture.path(), "remote.origin.promisor", OsStr::new("true"));
    git_config(
        fixture.path(),
        "remote.origin.partialclonefilter",
        OsStr::new("blob:none"),
    );
    let remote = format!("ext::{}", transport_script.display());
    git_config(fixture.path(), "remote.origin.url", OsStr::new(&remote));
    git_config(fixture.path(), "protocol.ext.allow", OsStr::new("always"));
    let credential = format!("!touch '{}'", credential_trap.display());
    git_config(fixture.path(), "credential.helper", OsStr::new(&credential));

    let missing_object = git_stdout(
        fixture.path(),
        &[OsStr::new("rev-parse"), OsStr::new("HEAD:src/add.ts")],
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
    let before = object_store_fingerprint(fixture.path());

    let snapshot = adapter(CollectionLimits::default())
        .collect(request(fixture.path()))
        .expect("a missing promised blob must remain a partial snapshot");
    let missing = snapshot
        .entries()
        .iter()
        .find(|entry| entry.path().as_bytes() == b"src/add.ts")
        .expect("the missing promised entry must remain listed");

    assert_eq!(snapshot.status(), EvidenceStatus::Partial);
    assert_eq!(missing.content().status(), EvidenceStatus::Unavailable);
    assert_eq!(
        missing.content().issue(),
        Some(ObjectIssue::MissingOrUnreadable)
    );
    assert!(!transport_trap.exists());
    assert!(!credential_trap.exists());
    assert_eq!(before, object_store_fingerprint(fixture.path()));
}

fn fixture() -> RepositoryFixture {
    RepositoryFixture::build(RepositoryScenario::TypeScriptProject)
        .expect("the deterministic fixture must build")
}

fn request(repository: &Path) -> SnapshotRequest<'_> {
    SnapshotRequest::new(repository, source(), OsStr::new("HEAD"))
}

fn source() -> RepositorySource {
    RepositorySource::local(
        ContentHash::from_str(
            "sha256:2222222222222222222222222222222222222222222222222222222222222222",
        )
        .expect("the test repository ID must be valid"),
    )
}

fn adapter(limits: CollectionLimits) -> GitCliAdapter {
    GitCliAdapter::from_trusted_executable(PathBuf::from("/usr/bin/git"), limits)
        .expect("the installed Git must satisfy the ADR baseline")
}

fn wrapper(body: &str) -> (TempDir, PathBuf) {
    let directory = tempfile::tempdir().expect("the wrapper directory must be creatable");
    let executable = directory.path().join("git-wrapper");
    write_wrapper(&executable, body);
    (directory, executable)
}

fn write_wrapper(executable: &Path, body: &str) {
    fs::write(executable, format!("#!/bin/sh\nset -eu\n{body}\n"))
        .expect("the wrapper must be writable");
    fs::set_permissions(executable, fs::Permissions::from_mode(0o755))
        .expect("the wrapper must be executable");
}

fn git_config(repository: &Path, key: &str, value: &OsStr) {
    let status = synthetic_git(repository)
        .args([
            OsStr::new("config"),
            OsStr::new("--local"),
            OsStr::new(key),
            value,
        ])
        .status()
        .expect("the synthetic Git config command must start");
    assert!(status.success());
}

fn git_stdout(repository: &Path, arguments: &[&OsStr]) -> Vec<u8> {
    let output = synthetic_git(repository)
        .args(arguments)
        .output()
        .expect("the synthetic Git metadata command must start");
    assert!(output.status.success());
    output.stdout
}

fn synthetic_git(repository: &Path) -> Command {
    let mut command = Command::new("/usr/bin/git");
    command
        .env_clear()
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .current_dir(repository);
    command
}

fn object_store_fingerprint(repository: &Path) -> Vec<(PathBuf, u64)> {
    fn visit(root: &Path, directory: &Path, output: &mut Vec<(PathBuf, u64)>) {
        let mut children = fs::read_dir(directory)
            .expect("the object directory must be readable")
            .collect::<Result<Vec<_>, _>>()
            .expect("object entries must be readable");
        children.sort_by_key(fs::DirEntry::file_name);
        for child in children {
            let path = child.path();
            let metadata = child.metadata().expect("object metadata must be readable");
            if metadata.is_dir() {
                visit(root, &path, output);
            } else if metadata.is_file() {
                output.push((
                    path.strip_prefix(root)
                        .expect("object paths remain under the object root")
                        .to_path_buf(),
                    metadata.len(),
                ));
            }
        }
    }

    let root = repository.join(".git/objects");
    let mut output = Vec::new();
    visit(&root, &root, &mut output);
    output
}
