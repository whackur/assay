use std::{fs, process::Command};

use assay_test_fixtures::{RepositoryFixture, RepositoryScenario};
use serde_json::Value;
use sha2::{Digest, Sha256};

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_assay")
}

fn fixed_command() -> Command {
    let mut command = Command::new(binary());
    command
        .env_clear()
        .env("ASSAY_TEST_FIXED_TIME", "2026-01-02T03:04:06Z");
    command
}

#[test]
fn capabilities_are_exact_schema_valid_and_do_not_claim_future_features() {
    let output = fixed_command()
        .args([
            "capabilities",
            "--format",
            "json",
            "--output",
            "-",
            "--no-color",
        ])
        .output()
        .expect("capabilities subprocess must start");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let golden: Value =
        serde_json::from_str(include_str!("../../../tests/golden/capabilities-v1.json")).unwrap();
    let mut expected = serde_json::to_vec_pretty(&golden).unwrap();
    expected.push(b'\n');
    assert_eq!(output.stdout, expected);
    assert!(!output.stdout.windows(2).any(|bytes| bytes == b"\x1b["));
}

#[test]
fn project_analyze_is_repeatable_private_local_and_has_a_reviewed_digest() {
    let fixture = RepositoryFixture::build(RepositoryScenario::TypeScriptProject)
        .expect("fixture must build");
    let run = || {
        fixed_command()
            .arg("project")
            .arg("analyze")
            .arg(fixture.path())
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
            .expect("analysis subprocess must start")
    };
    let first = run();
    let second = run();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(first.stderr.is_empty());
    assert_eq!(first.stdout, second.stdout);
    assert_eq!(first.stdout.last(), Some(&b'\n'));
    let digest = format!("{:x}", Sha256::digest(&first.stdout));
    assert_eq!(
        digest,
        include_str!("../../../tests/golden/cli/project-analyze-v1.sha256").trim()
    );

    let value: Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(value["schema_version"], "1.0.0");
    assert_eq!(value["manifest"]["status"], "partial");
    assert!(
        value["evidence"]
            .as_array()
            .is_some_and(|items| !items.is_empty())
    );
    assert!(
        value["evidence"]
            .as_array()
            .unwrap()
            .iter()
            .all(|item| item["privacy"]["visibility"] == "private_local")
    );
    let text = String::from_utf8(first.stdout).unwrap();
    assert!(!text.contains(&fixture.path().display().to_string()));
    assert!(!text.contains("fixture-author@example.invalid"));
    assert!(!text.contains("return left + right"));
    assert!(!text.contains("assay_score"));
    assert!(!text.contains("person"));
}

#[test]
fn invalid_input_and_missing_sources_use_stable_redacted_channels() {
    let unknown = fixed_command().arg("unknown-command").output().unwrap();
    assert_eq!(unknown.status.code(), Some(2));
    assert!(unknown.stdout.is_empty());
    assert!(!unknown.stderr.windows(2).any(|bytes| bytes == b"\x1b["));

    let secret_path = "/missing/private/token-should-not-appear";
    let missing = fixed_command()
        .args([
            "project",
            "analyze",
            secret_path,
            "--output",
            "-",
            "--no-color",
        ])
        .output()
        .unwrap();
    assert_eq!(missing.status.code(), Some(4));
    assert!(missing.stdout.is_empty());
    let diagnostic = String::from_utf8(missing.stderr).unwrap();
    assert_eq!(diagnostic, "error: source_not_found\n");
    assert!(!diagnostic.contains(secret_path));
}

#[test]
fn output_file_is_atomic_noclobber_and_stdout_stays_empty() {
    let fixture = RepositoryFixture::build(RepositoryScenario::MissingReadmeAndLicense)
        .expect("fixture must build");
    let temporary = tempfile::tempdir().unwrap();
    let destination = temporary.path().join("result.json");
    let run = || {
        fixed_command()
            .arg("project")
            .arg("analyze")
            .arg(fixture.path())
            .args(["--output"])
            .arg(&destination)
            .args(["--format", "json", "--no-color", "--non-interactive"])
            .output()
            .unwrap()
    };
    let first = run();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(first.stdout.is_empty());
    assert!(first.stderr.is_empty());
    let original = fs::read(&destination).unwrap();
    serde_json::from_slice::<Value>(&original).unwrap();

    let second = run();
    assert_eq!(second.status.code(), Some(12));
    assert!(second.stdout.is_empty());
    assert_eq!(fs::read(&destination).unwrap(), original);
    assert_eq!(
        String::from_utf8(second.stderr).unwrap(),
        "error: output_failed code=destination_exists\n"
    );
}

#[test]
fn default_repository_and_revision_are_accepted_in_a_non_tty() {
    let fixture = RepositoryFixture::build(RepositoryScenario::PythonProject).unwrap();
    let output = fixed_command()
        .current_dir(fixture.path())
        .args([
            "project",
            "analyze",
            "--output",
            "-",
            "--format",
            "json",
            "--no-color",
            "--non-interactive",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert!(serde_json::from_slice::<Value>(&output.stdout).is_ok());
}

#[test]
fn size_and_history_limits_remain_explicit_partial_evidence() {
    let fixture = RepositoryFixture::build(RepositoryScenario::DependencyOnlyChange).unwrap();
    let output = fixed_command()
        .env("ASSAY_TEST_MAX_OBJECT_BYTES", "1")
        .env("ASSAY_TEST_MAX_HISTORY_COMMITS", "1")
        .arg("project")
        .arg("analyze")
        .arg(fixture.path())
        .args([
            "--output",
            "-",
            "--format",
            "json",
            "--no-color",
            "--non-interactive",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["manifest"]["status"], "partial");
    assert_eq!(value["manifest"]["scope"]["history_status"], "partial");
    let evidence = value["evidence"].as_array().unwrap();
    assert!(
        evidence
            .iter()
            .any(|fact| fact["payload"]["kind"] == "tracked_file"
                && fact["payload"]["content_status"] == "partial"
                && fact["payload"]["content_hash"].is_null()
                && fact["payload"]["issue"] == "size_limit")
    );
}

#[cfg(unix)]
#[test]
fn gitlink_and_non_utf8_paths_survive_the_cli_contract() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};

    let fixture = RepositoryFixture::build(RepositoryScenario::TypeScriptProject).unwrap();
    let non_utf8 = fixture
        .path()
        .join(OsString::from_vec(b"src/nonutf-\xff.ts".to_vec()));
    fs::write(&non_utf8, b"export const opaque = true;\n").unwrap();
    let add = Command::new("/usr/bin/git")
        .current_dir(fixture.path())
        .arg("add")
        .arg("--")
        .arg(&non_utf8)
        .status()
        .unwrap();
    assert!(add.success());
    let commit_id = fixture.commit_ids().last().unwrap();
    let cache_info = format!("160000,{commit_id},deps/module");
    let gitlink = Command::new("/usr/bin/git")
        .current_dir(fixture.path())
        .args(["update-index", "--add", "--cacheinfo", &cache_info])
        .status()
        .unwrap();
    assert!(gitlink.success());
    let commit = Command::new("/usr/bin/git")
        .current_dir(fixture.path())
        .env("GIT_AUTHOR_DATE", "2001-02-05T06:07:08+09:00")
        .env("GIT_COMMITTER_DATE", "2001-02-05T06:07:08+09:00")
        .args(["commit", "--quiet", "-m", "Add opaque path and gitlink"])
        .status()
        .unwrap();
    assert!(commit.success());

    let output = fixed_command()
        .arg("project")
        .arg("analyze")
        .arg(fixture.path())
        .args([
            "--output",
            "-",
            "--format",
            "json",
            "--no-color",
            "--non-interactive",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    let evidence = value["evidence"].as_array().unwrap();
    let opaque_raw = evidence
        .iter()
        .find(|fact| {
            fact["payload"]["kind"] == "tracked_file"
                && fact["payload"]["path"]["encoding"] == "git_path_hex"
                && fact["payload"]["language_status"] == "unsupported"
        })
        .expect("opaque raw fact");
    let opaque_id = opaque_raw["id"].as_str().unwrap();
    assert!(evidence.iter().any(|fact| {
        fact["requested_kind"] == "file_classification"
            && fact["status"] == "unsupported"
            && fact["reason"] == "non_portable_path"
            && fact["related_evidence_ids"]
                .as_array()
                .is_some_and(|ids| ids.iter().any(|id| id == opaque_id))
            && fact["attempted_policy_version"].is_string()
    }));
    assert!(
        evidence
            .iter()
            .any(|fact| fact["payload"]["kind"] == "tracked_file"
                && fact["payload"]["mode"] == "gitlink"
                && fact["payload"]["object_kind"] == "commit"
                && fact["payload"]["content_status"] == "unsupported"
                && fact["payload"]["issue"] == "gitlink_content")
    );
}

#[test]
fn post_revision_collection_failure_is_exit_ten_not_not_found() {
    let fixture = RepositoryFixture::build(RepositoryScenario::TypeScriptProject).unwrap();
    let output = Command::new("/usr/bin/git")
        .current_dir(fixture.path())
        .args(["rev-parse", "HEAD^{tree}"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let tree = String::from_utf8(output.stdout).unwrap();
    let tree = tree.trim();
    let object = fixture
        .path()
        .join(".git/objects")
        .join(&tree[..2])
        .join(&tree[2..]);
    fs::remove_file(object).unwrap();

    let output = fixed_command()
        .arg("project")
        .arg("analyze")
        .arg(fixture.path())
        .args([
            "--output",
            "-",
            "--format",
            "json",
            "--no-color",
            "--non-interactive",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(10));
    assert!(output.stdout.is_empty());
    let diagnostic = String::from_utf8(output.stderr).unwrap();
    assert!(
        diagnostic.starts_with("error: collection_failed stage="),
        "{diagnostic}"
    );
    assert!(!diagnostic.contains("source_or_revision_not_found"));
    assert!(!diagnostic.contains(fixture.path().to_string_lossy().as_ref()));
}
