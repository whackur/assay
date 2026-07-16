//! End-to-end coverage for local private history, the `--github-token-env`
//! contract, loopback `serve`, and administrator-only history operations.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Command, Stdio};

use assay_test_fixtures::{RepositoryFixture, RepositoryScenario};
use serde_json::Value;
use tempfile::TempDir;

const PLANTED_TOKEN: &str = "ghp_cli_leak_probe_0123456789";

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

// Spawns `serve --once` on an ephemeral loopback port, issues one GET, and
// returns the raw HTTP response.
fn serve_once_get(history: &std::path::Path, path: &str) -> String {
    let mut child = fixed_command()
        .arg("serve")
        .arg("--history")
        .arg(history)
        .args(["--port", "0", "--once"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("serve subprocess must start");

    let stderr = child.stderr.take().expect("serve stderr");
    let mut reader = BufReader::new(stderr);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .expect("serve announces address");
    assert!(line.contains("http://127.0.0.1:"), "not loopback: {line}");
    let address = line
        .trim()
        .rsplit("http://")
        .next()
        .expect("address token")
        .to_owned();

    let mut client = TcpStream::connect(&address).expect("connect loopback");
    client
        .write_all(format!("GET {path} HTTP/1.1\r\nhost: localhost\r\n\r\n").as_bytes())
        .unwrap();
    let mut response = String::new();
    client.read_to_string(&mut response).unwrap();
    child.wait().expect("serve exits after one request");
    response
}

#[test]
fn analyze_records_private_history_without_leaking_token() {
    let fixture = RepositoryFixture::build(RepositoryScenario::TypeScriptProject)
        .expect("fixture must build");
    let history = TempDir::new().unwrap();

    let output = fixed_command()
        .env("GITHUB_TOKEN", PLANTED_TOKEN)
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
            "--github-token-env",
            "GITHUB_TOKEN",
        ])
        .arg("--record-history")
        .arg(history.path())
        .output()
        .expect("analyze subprocess must start");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let analysis: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(analysis["schema_version"], "1.0.0");

    // Neither stdout nor any on-disk record holds the token.
    assert!(!String::from_utf8_lossy(&output.stdout).contains(PLANTED_TOKEN));
    for entry in std::fs::read_dir(history.path().join("records")).unwrap() {
        let bytes = std::fs::read(entry.unwrap().path()).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        assert!(!text.contains(PLANTED_TOKEN), "token leaked into record");
        assert!(text.contains("private_local"));
    }

    // The loopback dashboard renders the immutable record.
    let index = serve_once_get(history.path(), "/api/history");
    assert!(index.starts_with("HTTP/1.1 200 OK"));
    assert!(index.contains("rec-000001"));
    let record = serve_once_get(history.path(), "/api/history/rec-000001");
    assert!(record.contains("\"visibility\":\"private_local\""));

    // Administrator soft-delete removes the record from the active dashboard.
    let deleted = fixed_command()
        .args(["history", "delete", "rec-000001", "--history"])
        .arg(history.path())
        .args(["--output", "-"])
        .output()
        .expect("history delete subprocess");
    assert!(deleted.status.success());
    let confirmation: Value = serde_json::from_slice(&deleted.stdout).unwrap();
    assert_eq!(confirmation["action"], "soft_delete");

    let after_delete = serve_once_get(history.path(), "/api/history/rec-000001");
    assert!(after_delete.starts_with("HTTP/1.1 404"));
}

#[test]
fn invalid_token_env_name_is_rejected_without_analysis() {
    let fixture = RepositoryFixture::build(RepositoryScenario::TypeScriptProject)
        .expect("fixture must build");
    let output = fixed_command()
        .arg("project")
        .arg("analyze")
        .arg(fixture.path())
        .args(["--github-token-env", "bad-name"])
        .output()
        .expect("analyze subprocess must start");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "error: invalid_input field=github_token_env\n"
    );
}
