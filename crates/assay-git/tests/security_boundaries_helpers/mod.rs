#![cfg(unix)]

use std::{
    ffi::OsStr,
    fs,
    io::Write,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
    str::FromStr,
};

use assay_domain::{ContentHash, RepositorySource};
use assay_git::{
    CollectionLimits, GitCliAdapter, RepositorySnapshot, RepositorySnapshotPort, SnapshotRequest,
};
use assay_test_fixtures::{RepositoryFixture, RepositoryScenario};
use tempfile::TempDir;

pub fn fixture() -> RepositoryFixture {
    RepositoryFixture::build(RepositoryScenario::TypeScriptProject)
        .expect("the deterministic fixture must build")
}

pub fn collect_with_raw_diff(
    repository: &Path,
    no_renames: &str,
    find_renames: &str,
) -> RepositorySnapshot {
    let body = format!(
        r#"
for argument in "$@"; do
  if [ "$argument" = "diff-tree" ]; then
    for mode in "$@"; do
      if [ "$mode" = "--find-renames=50%" ]; then
        printf '{find_renames}'
        exit 0
      fi
    done
    printf '{no_renames}'
    exit 0
  fi
done
exec /usr/bin/git "$@"
"#
    );
    let (_directory, executable) = wrapper(&body);
    let adapter = GitCliAdapter::from_trusted_executable(executable, CollectionLimits::default())
        .expect("the raw-diff wrapper must pass the capability probe");
    adapter
        .collect(request(repository))
        .expect("optional malformed deltas must not abort the repository snapshot")
}

pub fn request(repository: &Path) -> SnapshotRequest<'_> {
    SnapshotRequest::new(repository, source(), OsStr::new("HEAD"))
}

pub fn source() -> RepositorySource {
    RepositorySource::local(
        ContentHash::from_str(
            "sha256:2222222222222222222222222222222222222222222222222222222222222222",
        )
        .expect("the test repository ID must be valid"),
    )
}

pub fn adapter(limits: CollectionLimits) -> GitCliAdapter {
    GitCliAdapter::from_trusted_executable(PathBuf::from("/usr/bin/git"), limits)
        .expect("the installed Git must satisfy the ADR baseline")
}

pub fn wrapper(body: &str) -> (TempDir, PathBuf) {
    let directory = tempfile::tempdir().expect("the wrapper directory must be creatable");
    let executable = directory.path().join("git-wrapper");
    write_wrapper(&executable, body);
    (directory, executable)
}

pub fn write_wrapper(executable: &Path, body: &str) {
    // Write to a sibling temp file and atomically rename into place. Under
    // heavy thread pressure, executing a file that was just written via
    // O_TRUNC can hit ETXTBSY because the kernel still treats the inode as
    // open for writing. The rename-into-place pattern sidesteps that race:
    // the final executable path is never opened for writing.
    let staging = executable.with_extension("stage");
    let mut file = fs::File::create(&staging).expect("the staging wrapper must be creatable");
    file.write_all(format!("#!/bin/sh\nset -eu\n{body}\n").as_bytes())
        .expect("the wrapper must be writable");
    file.sync_all().expect("the wrapper must be synced");
    drop(file);
    fs::set_permissions(&staging, fs::Permissions::from_mode(0o755))
        .expect("the wrapper must be executable");
    fs::rename(&staging, executable).expect("the wrapper must be renamed into place");
}

pub fn git_config(repository: &Path, key: &str, value: &OsStr) {
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

pub fn git_stdout(repository: &Path, arguments: &[&OsStr]) -> Vec<u8> {
    let output = synthetic_git(repository)
        .args(arguments)
        .output()
        .expect("the synthetic Git metadata command must start");
    assert!(output.status.success());
    output.stdout
}

pub fn synthetic_git(repository: &Path) -> Command {
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

pub fn object_store_fingerprint(repository: &Path) -> Vec<(PathBuf, u64)> {
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
