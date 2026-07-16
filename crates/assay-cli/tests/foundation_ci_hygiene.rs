#![cfg(unix)]

use std::{
    collections::BTreeSet,
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
        let normalized_components = components
            .iter()
            .map(|component| component.to_ascii_lowercase().replace('_', "-"))
            .collect::<Vec<_>>();
        if components.iter().any(|component| {
            let normalized = component.to_ascii_lowercase().replace('_', "-");
            matches!(
                normalized.as_str(),
                "" | "."
                    | ".."
                    | ".git"
                    | ".orca"
                    | ".worktrees"
                    | "target"
                    | ".assay-cache"
                    | "source-clones"
                    | "source-cache"
                    | "repository-clones"
                    | "repository-cache"
                    | "private-eval"
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
                    // Foundation hygiene intentionally forbids checked-in build and cache
                    // directories. Runtime classifier fixtures create these paths only in
                    // temporary repositories, so the product source tree does not need them.
                    | ".cache"
                    | "node-modules"
                    | "dist"
                    | "build"
                    | ".next"
                    | "coverage"
                    | "out"
                    | ".turbo"
                    | "--pycache--"
                    | ".pytest-cache"
                    | ".mypy-cache"
                    | ".ruff-cache"
                    | ".tox"
                    | ".nox"
                    | "venv"
                    | ".venv"
                    | "virtualenv"
                    | ".virtualenv"
            )
        }) {
            return Err(format!("forbidden tracked directory: {path}"));
        }
        let name = components.last().copied().unwrap_or_default();
        if name == ".env.example" {
            continue;
        }
        let lower = name.to_ascii_lowercase();
        let extension = Path::new(&lower)
            .extension()
            .and_then(|value| value.to_str());
        let source_extension = extension.is_some_and(|extension| {
            matches!(extension, "rs" | "ts" | "tsx" | "js" | "jsx" | "py")
        });
        let sensitive_directory_index = normalized_components[..normalized_components.len() - 1]
            .iter()
            .position(|component| {
                matches!(
                    component.as_str(),
                    "auth"
                        | "auth-data"
                        | "auth-cache"
                        | "credential"
                        | "credentials"
                        | "credential-data"
                        | "credentials-data"
                        | "credential-cache"
                        | "credentials-cache"
                        | "token"
                        | "tokens"
                        | "token-data"
                        | "tokens-data"
                        | "token-cache"
                        | "tokens-cache"
                        | "secret"
                        | "secrets"
                        | "secret-data"
                        | "secrets-data"
                        | "secret-cache"
                        | "secrets-cache"
                )
            });
        if let Some(sensitive_index) = sensitive_directory_index {
            let source_artifact = source_extension
                && normalized_components[..sensitive_index]
                    .iter()
                    .any(|component| component == "src");
            let public_context_index = normalized_components
                .iter()
                .position(|component| matches!(component.as_str(), "docs" | "examples"))
                .or_else(|| {
                    normalized_components
                        .windows(2)
                        .position(|pair| pair == ["fixtures", "public"])
                        .map(|index| index + 1)
                });
            let documented_or_public_example =
                public_context_index.is_some_and(|index| index < sensitive_index);
            if !source_artifact && !documented_or_public_example {
                return Err(format!("sensitive tracked data directory: {path}"));
            }
        }
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
        let sensitive_extension = extension.is_some_and(|extension| {
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

fn audit_staged_index(output: &[u8]) -> Result<(), String> {
    if output.is_empty() {
        return Ok(());
    }
    if output.last() != Some(&0) {
        return Err("staged index output is not NUL terminated".into());
    }
    let mut paths = Vec::new();
    let mut unique_paths = BTreeSet::new();
    for record in output[..output.len() - 1].split(|byte| *byte == 0) {
        if record.is_empty() {
            return Err("staged index output contains an empty record".into());
        }
        let separator = record
            .iter()
            .position(|byte| *byte == b'\t')
            .ok_or_else(|| "staged index record has no path separator".to_owned())?;
        let metadata = std::str::from_utf8(&record[..separator])
            .map_err(|_| "staged index metadata is not ASCII".to_owned())?;
        let mut fields = metadata.split(' ');
        let mode = fields
            .next()
            .ok_or_else(|| "staged index mode is missing".to_owned())?;
        let object_id = fields
            .next()
            .ok_or_else(|| "staged index object ID is missing".to_owned())?;
        let stage = fields
            .next()
            .ok_or_else(|| "staged index stage is missing".to_owned())?;
        if fields.next().is_some() || mode.is_empty() || object_id.is_empty() || stage.is_empty() {
            return Err("staged index metadata has an invalid field count".into());
        }
        match mode {
            "100644" | "100755" | "120000" => {}
            "160000" => return Err("tracked gitlinks are forbidden".into()),
            _ => return Err("staged index mode is invalid".into()),
        }
        if !matches!(object_id.len(), 40 | 64)
            || !object_id
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            || object_id.bytes().all(|byte| byte == b'0')
        {
            return Err("staged index object ID is invalid".into());
        }
        if stage != "0" {
            return Err("unmerged staged index entries are forbidden".into());
        }
        let path = std::str::from_utf8(&record[separator + 1..])
            .map_err(|_| "tracked path is not UTF-8".to_owned())?;
        if path.is_empty() || !unique_paths.insert(path) {
            return Err("staged index path is empty or duplicated".into());
        }
        paths.push(path);
    }
    audit_tracked_paths(&paths)
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
        "credentials/provider.json",
        "auth/session.rs",
        "Auth/session.json",
        "secrets/examples/value.json",
        "tokens/cache.json",
        "TOKENS_DATA/cache.json",
        "secrets/value.json",
        "configs/auth/session.json",
        "data/credentials/provider.json",
        "cache/secrets/value.json",
        ".cache/result.json",
        "Node_Modules/package/index.js",
        "dist/bundle.js",
        "build/output.bin",
        ".next/server/app.js",
        "coverage/lcov.info",
        "out/report.json",
        ".turbo/state.json",
        "python/__pycache__/module.pyc",
        ".pytest_cache/state",
        "venv/bin/python",
        ".venv/bin/python",
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
        "crates/assay-identity/src/auth/session.rs",
        "web/src/auth/session.ts",
        "docs/auth/session.md",
        "examples/credentials/provider.json",
        "tests/fixtures/public/tokens/example.json",
        "docs/credentials.md",
    ])
    .expect("explicit public examples and ordinary source names stay allowed");
}

#[test]
fn staged_index_audit_rejects_gitlinks_and_malformed_records() {
    let oid = "a".repeat(40);
    let sha256_oid = "b".repeat(64);
    let normal = format!(
        "100644 {oid} 0\tREADME.md\0\
         100755 {oid} 0\tscripts/check.sh\0\
         120000 {oid} 0\tcurrent-docs\0\
         100644 {sha256_oid} 0\tsha256-object.txt\0"
    );
    audit_staged_index(normal.as_bytes()).expect("normal blobs, executables, and symlinks");

    let gitlink = format!("160000 {oid} 0\tordinary/path\0");
    assert!(audit_staged_index(gitlink.as_bytes()).is_err());
    for malformed in [
        format!("100644 {oid} 0\tmissing-nul"),
        format!("100644 {oid} 0 no-tab\0"),
        "100644 invalid 0\tbad-object\0".to_owned(),
        format!("100644 {oid} 1\tunmerged\0"),
        format!("100664 {oid} 0\tbad-mode\0"),
        format!("100644 {oid} 0\tduplicate\0100644 {oid} 0\tduplicate\0"),
        format!("100644 {oid} 0\tfirst\0\0100644 {oid} 0\tsecond\0"),
    ] {
        assert!(
            audit_staged_index(malformed.as_bytes()).is_err(),
            "malformed stage record was accepted: {malformed:?}"
        );
    }
}

#[test]
fn staged_index_audit_rejects_a_real_gitlink_entry() {
    let temporary = tempfile::tempdir().expect("temporary repository");
    successful(
        git_command(temporary.path())
            .args(["init", "--quiet"])
            .output()
            .expect("git init"),
        "git init",
    );
    successful(
        git_command(temporary.path())
            .args([
                "update-index",
                "--add",
                "--cacheinfo",
                "160000,1111111111111111111111111111111111111111,nested/repository",
            ])
            .output()
            .expect("git update-index"),
        "git update-index",
    );
    let staged = successful(
        git_command(temporary.path())
            .args(["ls-files", "--stage", "-z"])
            .output()
            .expect("git ls-files"),
        "git ls-files",
    );
    assert!(staged.stdout.starts_with(b"160000 "));
    assert!(audit_staged_index(&staged.stdout).is_err());
}
