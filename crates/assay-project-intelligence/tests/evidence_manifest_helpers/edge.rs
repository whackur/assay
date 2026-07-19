use std::{ffi::OsStr, fs, os::unix::ffi::OsStringExt, path::Path, process::Command};

use assay_domain::RepositorySource;
use assay_git::{CollectionLimits, RepositorySnapshot};

use super::common::{collect_snapshot, source, trusted_git};

pub fn edge_snapshot() -> RepositorySnapshot {
    let directory = tempfile::tempdir().unwrap();
    let repository = directory.path().join("edge-repository");
    git(
        directory.path(),
        [
            "init",
            "--quiet",
            "--initial-branch=main",
            repository.to_str().unwrap(),
        ],
    );
    git(&repository, ["config", "user.name", "Assay Fixture"]);
    git(
        &repository,
        ["config", "user.email", "fixture@example.invalid"],
    );
    fs::write(repository.join("README.md"), b"# Edge fixture\n").unwrap();
    for (name, contents) in [
        (b"src/non-utf8-\xfe.ts".as_slice(), b"first\n".as_slice()),
        (b"src/non-utf8-\xff.ts".as_slice(), b"second\n".as_slice()),
    ] {
        let invalid_path = repository.join(std::ffi::OsString::from_vec(name.to_vec()));
        fs::create_dir_all(invalid_path.parent().unwrap()).unwrap();
        fs::write(invalid_path, contents).unwrap();
    }
    fs::write(
        repository.join("src/copy-a.ts"),
        b"export const duplicated = true;\n",
    )
    .unwrap();
    fs::write(
        repository.join("src/copy-b.ts"),
        b"export const duplicated = true;\n",
    )
    .unwrap();
    git(&repository, ["add", "--all", "--", "."]);
    git(&repository, ["commit", "--quiet", "-m", "Add edge paths"]);
    let target = git_output(&repository, ["rev-parse", "HEAD"]);
    git(
        &repository,
        [
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{target},deps/module"),
        ],
    );
    git(&repository, ["commit", "--quiet", "-m", "Add gitlink"]);
    collect_snapshot(
        &repository,
        OsStr::new("HEAD"),
        source(),
        CollectionLimits::default(),
    )
}

fn git<const N: usize>(directory: &Path, arguments: [&str; N]) {
    let status = Command::new(trusted_git())
        .current_dir(directory)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .args(arguments)
        .status()
        .unwrap();
    assert!(status.success());
}

fn git_output<const N: usize>(directory: &Path, arguments: [&str; N]) -> String {
    let output = Command::new(trusted_git())
        .current_dir(directory)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .args(arguments)
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}
