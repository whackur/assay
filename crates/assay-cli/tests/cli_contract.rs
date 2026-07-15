use std::{
    fs,
    io::Write,
    process::{Command, Stdio},
};

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
    let portable_partial_classifications = value["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|fact| {
            fact["status"] == "partial"
                && fact["payload"]["kind"] == "file_classification"
                && fact["payload"]["reason"] == "attributes_unavailable"
        })
        .map(|fact| fact["id"].clone())
        .collect::<Vec<_>>();
    assert!(!portable_partial_classifications.is_empty());
    let attribute_limitation = value["manifest"]["limitations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|limitation| limitation["code"] == "attribute_resolution_unavailable")
        .expect("portable partial classifications require an attribute limitation");
    assert_eq!(
        attribute_limitation["affected_evidence_ids"],
        Value::Array(portable_partial_classifications)
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
fn overlong_git_path_becomes_citable_partial_evidence_without_path_disclosure() {
    let temporary = tempfile::tempdir().unwrap();
    let repository = temporary.path().join("long-path.git");
    let init = Command::new("/usr/bin/git")
        .args(["init", "--bare", "--quiet"])
        .arg(&repository)
        .status()
        .unwrap();
    assert!(init.success());

    let segment = format!("private-long-component-{}", "x".repeat(76));
    let opaque_prefix = vec![segment.as_str(); 110].join("/");
    let matching_path = format!("{opaque_prefix}/LICENSE");
    let unrelated_path = format!("{opaque_prefix}/secret.ts");
    assert!(matching_path.len() > 10_000);
    assert!(unrelated_path.len() > 10_000);
    let stream = format!(
        "blob\nmark :1\ndata 2\nx\nblob\nmark :2\ndata 2\ny\nblob\nmark :3\ndata 2\nz\ncommit refs/heads/main\nmark :4\nauthor Fixture <fixture@example.invalid> 981173106 +0000\ncommitter Fixture <fixture@example.invalid> 981173106 +0000\ndata 4\nlong\nM 100644 :1 {matching_path}\nM 100644 :2 {unrelated_path}\nM 100644 :3 README.md\n\ndone\n"
    );
    let mut importer = Command::new("/usr/bin/git")
        .current_dir(&repository)
        .args(["fast-import", "--quiet"])
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    importer
        .stdin
        .take()
        .unwrap()
        .write_all(stream.as_bytes())
        .unwrap();
    assert!(importer.wait().unwrap().success());
    assert!(
        Command::new("/usr/bin/git")
            .current_dir(&repository)
            .args(["symbolic-ref", "HEAD", "refs/heads/main"])
            .status()
            .unwrap()
            .success()
    );

    let output = fixed_command()
        .arg("project")
        .arg("analyze")
        .arg(&repository)
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
    let evidence = value["evidence"].as_array().unwrap();
    let raw = evidence
        .iter()
        .filter(|fact| {
            fact["requested_kind"] == "tracked_file" && fact["reason"] == "path_length_limit"
        })
        .collect::<Vec<_>>();
    assert_eq!(raw.len(), 2);
    assert!(raw.iter().all(|fact| fact["status"] == "unsupported"));
    let raw_ids = raw
        .iter()
        .map(|fact| fact["id"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    let classifications = evidence
        .iter()
        .filter(|fact| {
            fact["requested_kind"] == "file_classification" && fact["reason"] == "path_length_limit"
        })
        .collect::<Vec<_>>();
    assert_eq!(classifications.len(), 2);
    assert!(classifications.iter().all(|fact| {
        fact["status"] == "unsupported"
            && fact["attempted_policy_version"].is_string()
            && fact["related_evidence_ids"].as_array().is_some_and(|ids| {
                ids.len() == 1 && raw_ids.contains(&ids[0].as_str().unwrap().to_owned())
            })
    }));
    let classification_ids = classifications
        .iter()
        .map(|fact| fact["id"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    let mut cited_raw_ids = classifications
        .iter()
        .map(|fact| fact["related_evidence_ids"][0].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    cited_raw_ids.sort();
    assert_eq!(cited_raw_ids, raw_ids);
    assert!(
        value["manifest"]["limitations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|limitation| {
                limitation["code"] == "path_length_limit"
                    && limitation["affected_evidence_ids"]
                        .as_array()
                        .is_some_and(|ids| {
                            raw_ids
                                .iter()
                                .all(|expected| ids.iter().any(|id| id == expected))
                                && classification_ids
                                    .iter()
                                    .all(|expected| ids.iter().any(|id| id == expected))
                        })
            })
    );
    assert!(
        value["manifest"]["limitations"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|limitation| limitation["code"] == "attribute_resolution_unavailable")
            .all(|limitation| {
                !limitation["affected_evidence_ids"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|id| classification_ids.iter().any(|expected| id == expected))
            }),
        "a path-limited public classification is not partial attribute evidence"
    );
    let license = evidence
        .iter()
        .find(|fact| fact["payload"]["feature"] == "license")
        .expect("license feature evidence");
    assert_eq!(license["payload"]["state"], "unavailable");
    assert_eq!(
        license["payload"]["related_evidence_ids"],
        serde_json::json!(raw_ids)
    );
    let generated = evidence
        .iter()
        .find(|fact| fact["payload"]["feature"] == "generated_content")
        .expect("generated-content feature evidence");
    assert_eq!(generated["payload"]["state"], "unavailable");
    let incomplete_classification_ids = evidence
        .iter()
        .filter(|fact| {
            (fact["payload"]["kind"] == "file_classification"
                || fact["requested_kind"] == "file_classification")
                && fact["status"] != "complete"
        })
        .map(|fact| fact["id"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(
        generated["payload"]["related_evidence_ids"],
        serde_json::json!(incomplete_classification_ids)
    );
    let readme_raw_id = evidence
        .iter()
        .find(|fact| {
            fact["payload"]["kind"] == "tracked_file"
                && fact["payload"]["path"]["value"] == "README.md"
        })
        .unwrap()["id"]
        .as_str()
        .unwrap();
    let readme = evidence
        .iter()
        .find(|fact| fact["payload"]["feature"] == "readme")
        .unwrap();
    assert_eq!(readme["payload"]["state"], "present");
    assert_eq!(
        readme["payload"]["related_evidence_ids"],
        serde_json::json!([readme_raw_id])
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(!text.contains("private-long-component"));
    assert!(!text.contains(&matching_path));
    assert!(!text.contains(&unrelated_path));
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
