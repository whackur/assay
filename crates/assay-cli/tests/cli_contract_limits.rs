mod cli_contract_common;

use std::{
    fs,
    io::Write,
    process::{Command, Stdio},
};

use assay_test_fixtures::{RepositoryFixture, RepositoryScenario};
use serde_json::Value;

use cli_contract_common::{fixed_command, trusted_git};

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

#[test]
fn overlong_git_path_becomes_citable_partial_evidence_without_path_disclosure() {
    let temporary = tempfile::tempdir().unwrap();
    let repository = temporary.path().join("long-path.git");
    let init = Command::new(trusted_git())
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
    let mut importer = Command::new(trusted_git())
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
        Command::new(trusted_git())
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
    let output = Command::new(trusted_git())
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
