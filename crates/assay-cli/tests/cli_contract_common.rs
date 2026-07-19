#![allow(dead_code)]

use std::process::Command;

use assay_test_fixtures::trusted_git_executable;

pub fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_assay")
}

pub fn trusted_git() -> std::path::PathBuf {
    trusted_git_executable().expect("tests require a deployment-trusted Git executable")
}

pub fn fixed_command() -> Command {
    let mut command = Command::new(binary());
    command
        .env_clear()
        .env("ASSAY_TEST_FIXED_TIME", "2026-01-02T03:04:06Z");
    command
}
