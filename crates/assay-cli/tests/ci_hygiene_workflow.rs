#![cfg(unix)]
//! CI workflow contract and repository hygiene tests.

mod ci_hygiene;

use ci_hygiene::audit::audit_staged_index;
use ci_hygiene::common::{git_command, repository_root, successful};
use ci_hygiene::yaml::audit_ci_workflow;

use std::fs;

use audit::audit_staged_index;
use common::{git_command, repository_root, successful};
use yaml::audit_ci_workflow;

#[test]
fn active_ci_contract_is_read_only_pinned_and_runs_the_local_gates() {
    let root = repository_root();
    let workflow = fs::read_to_string(root.join(".github/workflows/ci.yml"))
        .expect("the foundation milestone requires CI");
    audit_ci_workflow(&workflow).expect("active fail-closed CI workflow");

    let ignore = fs::read_to_string(root.join(".gitignore")).expect("root gitignore");
    for entry in [
        ".orca/",
        ".worktrees/",
        "target/",
        ".assay-cache/",
        ".env",
        ".env.*",
        "!.env.example",
        "!**/.env.example",
    ] {
        assert!(
            ignore.lines().any(|line| line == entry),
            "missing ignore `{entry}`"
        );
    }

    let tracked = successful(
        git_command(&root)
            .args(["ls-files", "--stage", "-z"])
            .output()
            .expect("git ls-files"),
        "git ls-files",
    );
    audit_staged_index(&tracked.stdout).expect("tracked repository modes and paths stay hygienic");
    assert!(
        tracked
            .stdout
            .windows(b"\tCargo.lock\0".len())
            .any(|window| window == b"\tCargo.lock\0")
    );
}

#[test]
fn ci_audit_rejects_job_permission_overrides_and_commented_commands() {
    let workflow = fs::read_to_string(repository_root().join(".github/workflows/ci.yml")).unwrap();
    let write_override = workflow.replace(
        "    name: Rust and schema contracts\n",
        "    name: Rust and schema contracts\n    permissions: write-all\n",
    );
    assert!(write_override.contains("permissions: write-all"));
    assert!(audit_ci_workflow(&write_override).is_err());

    let commented_command = workflow.replace(
        "        run: cargo fmt --check",
        "        # run: cargo fmt --check",
    );
    assert!(commented_command.contains("cargo fmt --check"));
    assert!(audit_ci_workflow(&commented_command).is_err());
}
