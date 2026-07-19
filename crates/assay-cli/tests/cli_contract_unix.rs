#![cfg(unix)]

mod cli_contract_common;

use std::{ffi::OsString, fs, os::unix::ffi::OsStringExt, process::Command};

use assay_test_fixtures::{RepositoryFixture, RepositoryScenario};
use serde_json::Value;

use cli_contract_common::fixed_command;

#[test]
fn gitlink_and_non_utf8_paths_survive_the_cli_contract() {
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
