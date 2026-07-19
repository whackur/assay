#![cfg(unix)]
//! Shared helpers for the foundation vertical slice integration tests.

use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
};

pub(crate) const FIXED_TIME: &str = "2026-01-02T03:04:06Z";
pub(crate) const SECRET_MARKER: &str = "VER001_PRIVATE_SOURCE_TOKEN_DO_NOT_PUBLISH";
pub(crate) const REPOSITORY_EXECUTION_SENTINELS: [&str; 7] = [
    "TRIPWIRE_PREINSTALL",
    "TRIPWIRE_INSTALL",
    "TRIPWIRE_POSTINSTALL",
    "TRIPWIRE_BUILD",
    "TRIPWIRE_TEST",
    "TRIPWIRE_JS_IMPORT",
    "TRIPWIRE_PYTHON_IMPORT",
];

pub(crate) fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("assay-cli must remain under crates/")
        .to_path_buf()
}

pub(crate) fn assay_command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_assay"));
    command.env_clear().env("ASSAY_TEST_FIXED_TIME", FIXED_TIME);
    command
}

pub(crate) fn git_command(repository: &Path) -> Command {
    let mut command = Command::new("/usr/bin/git");
    command
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LC_ALL", "C")
        .env("TZ", "UTC")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_ATTR_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .current_dir(repository);
    command
}

pub(crate) fn successful(output: Output, operation: &str) -> Output {
    assert!(
        output.status.success(),
        "{operation} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}
