#![cfg(unix)]
//! Runs the foundation analysis subprocess against a fixture.

use std::process::Output;

use super::common::assay_command;
use super::fixture::FoundationFixture;

pub(crate) fn run_analysis(fixture: &FoundationFixture) -> Output {
    assay_command()
        .env("PATH", &fixture.command_shims)
        .arg("project")
        .arg("analyze")
        .arg(&fixture.repository)
        .args([
            "--revision",
            "HEAD",
            "--evaluator",
            "deterministic",
            "--format",
            "json",
            "--output",
            "-",
            "--no-color",
            "--non-interactive",
        ])
        .output()
        .expect("foundation analysis subprocess")
}
