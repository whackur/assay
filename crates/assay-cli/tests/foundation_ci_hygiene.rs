#![cfg(unix)]

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("assay-cli must remain under crates/")
        .to_path_buf()
}

fn git_command(repository: &Path) -> Command {
    let mut command = Command::new("/usr/bin/git");
    command
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LC_ALL", "C")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .current_dir(repository);
    command
}

fn successful(output: Output, operation: &str) -> Output {
    assert!(
        output.status.success(),
        "{operation} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

#[derive(Debug, Eq, PartialEq)]
struct ActiveYamlLine {
    indent: usize,
    text: String,
}

fn active_yaml_lines(workflow: &str) -> Result<Vec<ActiveYamlLine>, String> {
    workflow
        .lines()
        .filter_map(|line| {
            if line.contains('\t') {
                return Some(Err("workflow indentation contains a tab".into()));
            }
            let text = line.trim_start();
            if text.is_empty() || text.starts_with('#') {
                return None;
            }
            if text.contains(" #") {
                return Some(Err("inline workflow comments are ambiguous".into()));
            }
            let indent = line.len() - text.len();
            if indent % 2 != 0 {
                return Some(Err("workflow indentation must use two-space levels".into()));
            }
            Some(Ok(ActiveYamlLine {
                indent,
                text: text.to_owned(),
            }))
        })
        .collect()
}

fn audit_ci_workflow(workflow: &str) -> Result<(), String> {
    const EXPECTED_ACTIVE_WORKFLOW: &str = r#"name: CI
on:
  pull_request:
  push:
    branches: [main]
permissions:
  contents: read
jobs:
  rust:
    name: Rust and schema contracts
    runs-on: ubuntu-24.04
    env:
      CARGO_TERM_COLOR: never
    steps:
      - name: Check out the repository
        uses: actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683
        with:
          persist-credentials: false
      - name: Verify the Git adapter baseline
        shell: bash
        run: |
          git version --build-options
          version="$(git version | awk '{print $3}')"
          dpkg --compare-versions "$version" ge 2.47.0
      - name: Install the pinned Rust toolchain
        run: rustup toolchain install 1.97.0 --profile minimal --component rustfmt --component clippy
      - name: Restore Cargo dependency cache
        uses: actions/cache@5a3ec84eff668545956fd18022155c47e93e2684
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
      - name: Check formatting
        run: cargo fmt --check
      - name: Lint without warnings
        run: cargo clippy --workspace --all-targets --all-features -- -D warnings
      - name: Run workspace tests
        run: cargo test --workspace
      - name: Validate public schemas and goldens
        run: cargo test -p assay-cli --test schema_contracts
"#;
    let active = active_yaml_lines(workflow)?;
    let expected = active_yaml_lines(EXPECTED_ACTIVE_WORKFLOW)?;
    if active != expected {
        return Err(format!(
            "active workflow structure differs from the read-only contract\nactual: {active:#?}\nexpected: {expected:#?}"
        ));
    }
    Ok(())
}

fn audit_tracked_paths(paths: &[&str]) -> Result<(), String> {
    for path in paths {
        if path.is_empty() || path.starts_with('/') || path.contains('\\') {
            return Err(format!("forbidden tracked path: {path}"));
        }
        let components = path.split('/').collect::<Vec<_>>();
        if components.iter().any(|component| {
            let normalized = component.replace('_', "-");
            matches!(
                *component,
                "" | "."
                    | ".."
                    | ".git"
                    | ".orca"
                    | ".worktrees"
                    | "target"
                    | ".assay-cache"
                    | "source-clones"
                    | "source_clones"
                    | "source-cache"
                    | "repository-clones"
                    | "repository_clones"
                    | "repository-cache"
            ) || matches!(
                normalized.as_str(),
                "private-eval"
                    | "private-evals"
                    | "private-evaluation"
                    | "private-evaluations"
                    | "private-eval-data"
                    | "private-evals-data"
                    | "private-evaluation-data"
                    | "private-evaluations-data"
                    | "private-data"
                    | "private-dataset"
                    | "private-datasets"
            )
        }) {
            return Err(format!("forbidden tracked directory: {path}"));
        }
        let name = components.last().copied().unwrap_or_default();
        if name == ".env.example" {
            continue;
        }
        let lower = name.to_ascii_lowercase();
        let sensitive_name = lower == ".env"
            || lower.starts_with(".env.")
            || matches!(
                lower.as_str(),
                "auth.json"
                    | "credentials"
                    | "credentials.json"
                    | "credentials.toml"
                    | "credentials.yaml"
                    | "credentials.yml"
                    | "token"
                    | "token.json"
                    | "token.txt"
                    | "token.toml"
                    | "token.yaml"
                    | "token.yml"
                    | "id_rsa"
                    | "id_ed25519"
                    | "private_key"
                    | "private-key"
                    | "access_token"
                    | "access-token"
                    | "refresh_token"
                    | "refresh-token"
                    | "api_token"
                    | "api-token"
                    | "private-evaluation.json"
            );
        let sensitive_extension = Path::new(&lower)
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|extension| {
                matches!(
                    extension,
                    "pem" | "key" | "p12" | "pfx" | "secret" | "token" | "credentials"
                )
            });
        if sensitive_name || sensitive_extension {
            return Err(format!("sensitive tracked file: {path}"));
        }
    }
    Ok(())
}

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
            .args(["ls-files", "-z"])
            .output()
            .expect("git ls-files"),
        "git ls-files",
    );
    let tracked = tracked
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| String::from_utf8_lossy(path))
        .collect::<Vec<_>>();
    assert!(tracked.iter().any(|path| path.as_ref() == "Cargo.lock"));
    let tracked_refs = tracked.iter().map(AsRef::as_ref).collect::<Vec<_>>();
    audit_tracked_paths(&tracked_refs).expect("tracked repository hygiene");
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

#[test]
fn tracked_path_audit_rejects_sensitive_mutations_without_blocking_public_examples() {
    for forbidden in [
        ".env",
        "nested/.env.local",
        ".assay-cache/result.json",
        "config/auth.json",
        "credentials.json",
        "deploy/private.key",
        "tokens/access.token",
        "private-evaluation/run.json",
        "private-evaluations/run.json",
        "private_evaluation_data/run.json",
        "private-evaluations-data/run.json",
        "private-data/run.json",
        "private_datasets/run.json",
        "source-clones/repository/README.md",
        "nested/.git/config",
    ] {
        assert!(
            audit_tracked_paths(&[forbidden]).is_err(),
            "sensitive mutation was accepted: {forbidden}"
        );
    }
    audit_tracked_paths(&[
        ".env.example",
        "examples/.env.example",
        "configs/example.toml",
        "tests/fixtures/public-key.example",
        "src/token.rs",
        "docs/credentials.md",
    ])
    .expect("explicit public examples and ordinary source names stay allowed");
}
