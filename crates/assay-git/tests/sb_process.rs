#![cfg(unix)]

mod security_boundaries_helpers;

use assay_git::{
    CollectionErrorKind, CollectionLimits, CollectionStage, GitCliAdapter, RepositorySnapshotPort,
};
use security_boundaries_helpers as helpers;
use serial_test::serial;
use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

#[test]
#[serial]
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
    helpers::write_wrapper(&executable, &body);
    let limits = CollectionLimits {
        command_timeout: Duration::from_millis(100),
        ..CollectionLimits::default()
    };
    let adapter = GitCliAdapter::from_trusted_executable(executable, limits)
        .expect("the orphan wrapper must pass the capability probe");
    let fixture = helpers::fixture();
    let started = Instant::now();
    let error = adapter
        .collect(helpers::request(fixture.path()))
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
#[serial]
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
    let (_directory, timeout_git) = helpers::wrapper(timeout_script);
    let limits = CollectionLimits {
        command_timeout: Duration::from_millis(100),
        ..CollectionLimits::default()
    };
    let adapter = GitCliAdapter::from_trusted_executable(timeout_git, limits)
        .expect("the timeout wrapper must pass the capability probe");
    let fixture = helpers::fixture();
    let started = Instant::now();
    let error = adapter
        .collect(helpers::request(fixture.path()))
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
    let (_directory, pipe_git) = helpers::wrapper(pipe_script);
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
        .collect(helpers::request(fixture.path()))
        .expect_err("oversized stdout and stderr must fail without deadlock");
    assert_eq!(error.stage(), CollectionStage::ValidateObjectStore);
    assert_eq!(error.kind(), CollectionErrorKind::OutputLimit);
    assert!(started.elapsed() < Duration::from_secs(2));
}
