#![cfg(unix)]

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use jsonschema::{Draft, Resource};
use serde_json::Value;
use sha2::{Digest, Sha256};

const FIXED_TIME: &str = "2026-01-02T03:04:06Z";
const SECRET_MARKER: &str = "VER001_PRIVATE_SOURCE_TOKEN_DO_NOT_PUBLISH";
const REPOSITORY_EXECUTION_SENTINELS: [&str; 7] = [
    "TRIPWIRE_PREINSTALL",
    "TRIPWIRE_INSTALL",
    "TRIPWIRE_POSTINSTALL",
    "TRIPWIRE_BUILD",
    "TRIPWIRE_TEST",
    "TRIPWIRE_JS_IMPORT",
    "TRIPWIRE_PYTHON_IMPORT",
];

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("assay-cli must remain under crates/")
        .to_path_buf()
}

fn assay_command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_assay"));
    command.env_clear().env("ASSAY_TEST_FIXED_TIME", FIXED_TIME);
    command
}

fn git_command(repository: &Path) -> Command {
    let mut command = Command::new("/usr/bin/git");
    command
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LC_ALL", "C")
        .env("TZ", "UTC")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_ATTR_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
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

struct FoundationFixture {
    _temporary: tempfile::TempDir,
    repository: PathBuf,
    revision: String,
    tripwire: PathBuf,
    command_shims: PathBuf,
}

impl FoundationFixture {
    fn build() -> Self {
        let temporary = tempfile::tempdir().expect("fixture root");
        let repository = temporary.path().join("foundation repository");
        fs::create_dir(&repository).expect("fixture repository");
        successful(
            git_command(&repository)
                .args(["init", "--quiet", "--initial-branch=main", "--template="])
                .output()
                .expect("git init"),
            "git init",
        );
        for (key, value) in [
            ("user.name", "Assay Foundation Fixture"),
            ("user.email", "foundation-fixture@example.invalid"),
            ("commit.gpgSign", "false"),
            ("core.autocrlf", "false"),
        ] {
            successful(
                git_command(&repository)
                    .args(["config", "--local", key, value])
                    .output()
                    .expect("git config"),
                "git config",
            );
        }

        let files = BTreeMap::from([
            (
                ".gitattributes",
                b"generated/** linguist-generated=true\nvendor/** linguist-vendored=true\ndocs/untrusted.txt filter=assay diff=assay\n".as_slice(),
            ),
            (
                ".github/workflows/ci.yml",
                b"name: fixture-ci\non: [push]\njobs: {test: {runs-on: ubuntu-latest}}\n".as_slice(),
            ),
            ("LICENSE", b"MIT License\n".as_slice()),
            (
                "README.md",
                b"# Foundation Fixture\n\nStatic evidence only.\n".as_slice(),
            ),
            ("SECURITY.md", b"# Security Policy\n".as_slice()),
            ("config/app.toml", b"mode = \"fixture\"\n".as_slice()),
            ("coverage/lcov.info", b"TN:\nend_of_record\n".as_slice()),
            ("dist/bundle.js", b"const built = true;\n".as_slice()),
            ("docs/guide.md", b"# Guide\n".as_slice()),
            ("docs/untrusted.txt", SECRET_MARKER.as_bytes()),
            (
                "generated/client.pb.ts",
                b"export const generated = true;\n".as_slice(),
            ),
            ("infra/main.tf", b"terraform {}\n".as_slice()),
            ("migrations/001_init.sql", b"CREATE TABLE fixture(id INT);\n".as_slice()),
            ("native/unsupported.rs", b"pub fn unsupported() {}\n".as_slice()),
            (
                "python/import_tripwire.py",
                b"from pathlib import Path\nPath('TRIPWIRE_PYTHON_IMPORT').touch()\n".as_slice(),
            ),
            (
                "package-lock.json",
                b"{\"lockfileVersion\":3,\"packages\":{}}\n".as_slice(),
            ),
            (
                "package.json",
                b"{\"name\":\"foundation-fixture\",\"scripts\":{\"preinstall\":\": > TRIPWIRE_PREINSTALL\",\"install\":\": > TRIPWIRE_INSTALL\",\"postinstall\":\": > TRIPWIRE_POSTINSTALL\",\"build\":\": > TRIPWIRE_BUILD\",\"test\":\": > TRIPWIRE_TEST\"}}\n".as_slice(),
            ),
            (
                "src/import_tripwire.js",
                b"require('fs').writeFileSync('TRIPWIRE_JS_IMPORT', 'executed');\n".as_slice(),
            ),
            (
                "src/main.ts",
                b"export const foundation = (): string => \"private-source-body\";\n".as_slice(),
            ),
            (
                "tests/main.test.ts",
                b"import { foundation } from \"../src/main\";\nvoid foundation();\n".as_slice(),
            ),
            ("vendor/library.ts", b"export const vendored = true;\n".as_slice()),
        ]);
        for (relative, contents) in files {
            let destination = repository.join(relative);
            fs::create_dir_all(destination.parent().expect("fixture file parent"))
                .expect("fixture directory");
            fs::write(destination, contents).expect("fixture file");
        }
        successful(
            git_command(&repository)
                .args(["add", "--all"])
                .output()
                .expect("git add"),
            "git add",
        );
        successful(
            git_command(&repository)
                .env("GIT_AUTHOR_DATE", "2001-02-03T04:05:06+09:00")
                .env("GIT_COMMITTER_DATE", "2001-02-03T04:05:06+09:00")
                .args(["commit", "--quiet", "-m", "Add foundation evidence"])
                .output()
                .expect("git commit"),
            "git commit",
        );
        let revision = String::from_utf8(
            successful(
                git_command(&repository)
                    .args(["rev-parse", "HEAD"])
                    .output()
                    .expect("git rev-parse"),
                "git rev-parse",
            )
            .stdout,
        )
        .expect("ASCII revision")
        .trim()
        .to_owned();

        let tripwire = temporary.path().join("repository-code-executed");
        let trap = repository.join(".git/assay-tripwire.sh");
        fs::write(
            &trap,
            format!("#!/bin/sh\n: > '{}'\nexit 97\n", tripwire.display()),
        )
        .expect("tripwire script");
        let mut permissions = fs::metadata(&trap)
            .expect("tripwire metadata")
            .permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&trap, permissions).expect("tripwire permissions");
        let trap = trap.to_string_lossy().into_owned();
        let trap_command = format!("'{trap}'");
        for key in [
            "filter.assay.clean",
            "filter.assay.smudge",
            "diff.assay.textconv",
        ] {
            successful(
                git_command(&repository)
                    .args(["config", "--local", key, &trap_command])
                    .output()
                    .expect("hostile local config"),
                "hostile local config",
            );
        }
        let hook = repository.join(".git/hooks/post-checkout");
        fs::create_dir_all(hook.parent().expect("hook directory")).expect("hook directory");
        fs::write(&hook, format!("#!/bin/sh\nexec '{trap}'\n")).expect("hostile hook");
        let mut permissions = fs::metadata(&hook).expect("hook metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(hook, permissions).expect("hook permissions");
        let command_shims = temporary.path().join("command-shims");
        fs::create_dir(&command_shims).expect("command shim directory");
        for command in [
            "npm", "node", "npx", "build", "import", "python", "python3", "pip", "pip3", "cargo",
            "rustc", "make",
        ] {
            let shim = command_shims.join(command);
            fs::write(
                &shim,
                format!("#!/bin/sh\n: > '{}'\nexit 97\n", tripwire.display()),
            )
            .expect("command shim");
            let mut permissions = fs::metadata(&shim).expect("shim metadata").permissions();
            permissions.set_mode(0o700);
            fs::set_permissions(shim, permissions).expect("shim permissions");
        }
        let shim_probe = Command::new(command_shims.join("npm"))
            .output()
            .expect("shim self-check must execute");
        assert_eq!(shim_probe.status.code(), Some(97));
        assert!(tripwire.exists(), "command shim self-check is non-vacuous");
        fs::remove_file(&tripwire).expect("reset command shim tripwire");
        assert!(!tripwire.exists());
        for sentinel in REPOSITORY_EXECUTION_SENTINELS {
            assert!(!repository.join(sentinel).exists());
        }

        Self {
            _temporary: temporary,
            repository,
            revision,
            tripwire,
            command_shims,
        }
    }
}

fn project_analysis_validator() -> jsonschema::Validator {
    let root = repository_root();
    let read = |name: &str| {
        serde_json::from_slice::<Value>(
            &fs::read(root.join("schemas").join(name).join("v1.json"))
                .unwrap_or_else(|error| panic!("read {name} schema: {error}")),
        )
        .unwrap_or_else(|error| panic!("parse {name} schema: {error}"))
    };
    let schema = read("project-analysis");
    let resources = ["analysis-manifest", "project-evidence"]
        .into_iter()
        .map(|name| {
            let component = read(name);
            let id = component["$id"]
                .as_str()
                .expect("component schema ID")
                .to_owned();
            let resource = Resource::from_contents(component).expect("component resource");
            (id, resource)
        });
    jsonschema::options()
        .with_draft(Draft::Draft202012)
        .with_resources(resources)
        .should_validate_formats(true)
        .build(&schema)
        .expect("project-analysis validator")
}

fn run_analysis(fixture: &FoundationFixture) -> Output {
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

fn audit_bundle_citations(bundle: &Value) -> Result<(), String> {
    let evidence = bundle["evidence"]
        .as_array()
        .ok_or_else(|| "missing evidence".to_owned())?;
    let by_id = evidence
        .iter()
        .map(|record| {
            record["id"]
                .as_str()
                .map(|id| (id, record))
                .ok_or_else(|| "missing evidence ID".to_owned())
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    if by_id.len() != evidence.len() {
        return Err("duplicate evidence ID".into());
    }
    let require_reference = |reference: &Value, field: &str| -> Result<&Value, String> {
        let id = reference
            .as_str()
            .ok_or_else(|| format!("non-string citation in {field}"))?;
        by_id
            .get(id)
            .copied()
            .ok_or_else(|| format!("dangling citation in {field}: {id}"))
    };
    let require_references = |references: &Value, field: &str| -> Result<Vec<&Value>, String> {
        references
            .as_array()
            .ok_or_else(|| format!("missing citation list: {field}"))?
            .iter()
            .map(|reference| require_reference(reference, field))
            .collect()
    };
    for record in evidence {
        if let Some(related) = record.get("related_evidence_ids") {
            require_references(related, "evidence.related_evidence_ids")?;
        }
        let Some(payload) = record.get("payload") else {
            continue;
        };
        for field in ["related_evidence_ids", "implementation_evidence_ids"] {
            if let Some(references) = payload.get(field) {
                require_references(references, field)?;
            }
        }
        if let Some(source) = payload.get("source_evidence_id") {
            require_reference(source, "payload.source_evidence_id")?;
        }
        match payload["kind"].as_str() {
            Some("file_classification") => {
                let source = payload
                    .get("source_evidence_id")
                    .ok_or_else(|| "classification source citation missing".to_owned())?;
                let source_record = require_reference(source, "payload.source_evidence_id")?;
                let related = record["related_evidence_ids"]
                    .as_array()
                    .ok_or_else(|| "classification top-level relation missing".to_owned())?;
                if related.as_slice() != std::slice::from_ref(source) {
                    return Err("classification source relation mismatch".into());
                }
                let source_kind = source_record
                    .get("payload")
                    .and_then(|value| value["kind"].as_str())
                    .or_else(|| source_record["requested_kind"].as_str());
                if source_kind != Some("tracked_file") {
                    return Err("classification source is not tracked-file evidence".into());
                }
            }
            Some("repository_feature") => {
                let related = payload["related_evidence_ids"]
                    .as_array()
                    .ok_or_else(|| "feature citation list missing".to_owned())?;
                match payload["state"].as_str() {
                    Some("present" | "unavailable") if related.is_empty() => {
                        return Err("reviewable feature state has no citation".into());
                    }
                    Some("absent") if !related.is_empty() => {
                        return Err("absent feature has citations".into());
                    }
                    Some("present" | "unavailable" | "absent") => {}
                    _ => return Err("unknown repository feature state".into()),
                }
            }
            Some("claim_correspondence") => {
                require_references(
                    payload
                        .get("implementation_evidence_ids")
                        .ok_or_else(|| "claim citations missing".to_owned())?,
                    "payload.implementation_evidence_ids",
                )?;
            }
            _ => {}
        }
    }
    let manifest = bundle
        .get("manifest")
        .ok_or_else(|| "missing manifest".to_owned())?;
    for source in manifest["data_sources"]
        .as_array()
        .ok_or_else(|| "missing data sources".to_owned())?
    {
        require_reference(&source["id"], "manifest.data_sources.id")?;
    }
    for (kind, diagnostics) in [
        ("warning", &manifest["warnings"]),
        ("limitation", &manifest["limitations"]),
    ] {
        for diagnostic in diagnostics
            .as_array()
            .ok_or_else(|| format!("missing {kind} array"))?
        {
            let references = diagnostic
                .get("affected_evidence_ids")
                .ok_or_else(|| format!("{kind} citations missing"))?;
            let resolved = require_references(references, "diagnostic.affected_evidence_ids")?;
            if resolved.is_empty() {
                return Err(format!("{kind} citations empty"));
            }
        }
    }
    Ok(())
}

#[test]
fn fixed_repository_is_a_schema_valid_private_and_non_executing_vertical_slice() {
    let fixture = FoundationFixture::build();
    let first = run_analysis(&fixture);
    let second = run_analysis(&fixture);
    assert_eq!(first.status.code(), Some(0));
    assert_eq!(second.status.code(), Some(0));
    assert!(first.stderr.is_empty());
    assert!(second.stderr.is_empty());
    assert_eq!(first.stdout, second.stdout);
    assert!(!first.stdout.windows(2).any(|bytes| bytes == b"\x1b["));
    assert!(
        !fixture.tripwire.exists(),
        "Git filter, textconv, or hook ran"
    );
    for sentinel in REPOSITORY_EXECUTION_SENTINELS {
        assert!(!fixture.repository.join(sentinel).exists());
    }

    let digest = format!("{:x}", Sha256::digest(&first.stdout));
    let reviewed_digest = fs::read_to_string(
        repository_root().join("tests/golden/cli/foundation-vertical-slice-v1.sha256"),
    )
    .expect("reviewed foundation CLI digest");
    assert_eq!(digest, reviewed_digest.trim());

    let bundle: Value = serde_json::from_slice(&first.stdout).expect("single JSON result");
    audit_bundle_citations(&bundle).expect("closed evidence citations");
    let errors = project_analysis_validator()
        .iter_errors(&bundle)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(errors.is_empty(), "schema errors: {errors:#?}");
    assert_eq!(bundle["schema_version"], "1.0.0");
    let manifest = &bundle["manifest"];
    assert_eq!(manifest["analysis_version"], "repository-evidence-1");
    assert_eq!(
        manifest["rule_set_hash"],
        "sha256:23cc47cd5dc4a4e3f34cfb496daab541461d52d33572c9acc02f14a0cd4a34ae"
    );
    assert_eq!(
        manifest["config_hash"],
        "sha256:bb0850de816d8cb05caf9eda9c593ccc190aeed1873fbdd7d00cb72c18aba92e"
    );
    assert_eq!(manifest["generated_at"], FIXED_TIME);
    assert_eq!(manifest["source_snapshot"]["revision"], fixture.revision);
    assert_eq!(manifest["scope"]["head_revision"], fixture.revision);
    assert_eq!(manifest["scope"]["mode"], "single_revision");
    assert_eq!(manifest["status"], "partial");

    let evidence = bundle["evidence"].as_array().expect("evidence array");
    let evidence_ids = evidence
        .iter()
        .map(|record| record["id"].as_str().expect("evidence ID"))
        .collect::<BTreeSet<_>>();
    assert_eq!(evidence_ids.len(), evidence.len());
    let repository_id = manifest["source_snapshot"]["source"]["repository_id"]
        .as_str()
        .expect("portable repository ID");
    assert!(repository_id.starts_with("sha256:"));
    let allowed_statuses = BTreeSet::from([
        "complete",
        "partial",
        "unavailable",
        "unsupported",
        "insufficient",
        "pending",
    ]);
    let mut published_statuses = BTreeSet::new();
    for record in evidence {
        assert_eq!(record["repository"]["repository_id"], repository_id);
        assert_eq!(record["privacy"]["visibility"], "private_local");
        assert_eq!(record["privacy"]["source_content"], "not_retained");
        let status = record["status"].as_str().expect("string evidence status");
        assert!(allowed_statuses.contains(status));
        published_statuses.insert(status);
        if let Some(provenance) = record.get("provenance") {
            assert_eq!(provenance["repository_revision"], fixture.revision);
        }
        for related in record["related_evidence_ids"]
            .as_array()
            .into_iter()
            .flatten()
        {
            assert!(evidence_ids.contains(related.as_str().expect("related evidence ID")));
        }
    }
    assert!(published_statuses.contains("complete"));
    assert!(published_statuses.contains("partial"));
    for source in manifest["data_sources"].as_array().expect("data sources") {
        assert_eq!(source["revision"], fixture.revision);
        assert!(allowed_statuses.contains(source["status"].as_str().expect("data-source status")));
        assert!(evidence_ids.contains(source["id"].as_str().expect("source evidence ID")));
    }
    assert!(
        allowed_statuses.contains(
            manifest["scope"]["history_status"]
                .as_str()
                .expect("history status")
        )
    );

    let raw_ids = evidence
        .iter()
        .filter(|record| record["payload"]["kind"] == "tracked_file")
        .map(|record| record["id"].as_str().expect("raw ID"))
        .collect::<BTreeSet<_>>();
    let classifications = evidence
        .iter()
        .filter(|record| record["payload"]["kind"] == "file_classification")
        .collect::<Vec<_>>();
    assert_eq!(raw_ids.len(), 21);
    assert_eq!(classifications.len(), raw_ids.len());
    assert!(classifications.iter().all(|record| {
        record["payload"]["source_evidence_id"]
            .as_str()
            .is_some_and(|id| raw_ids.contains(id))
            && record["attempted_policy_version"] == "file-classifier-1"
    }));
    let categories = classifications
        .iter()
        .filter_map(|record| record["payload"]["classification"]["primary_category"].as_str())
        .collect::<BTreeSet<_>>();
    let expected_categories = BTreeSet::from([
        "build_output",
        "ci_cd",
        "configuration",
        "coverage",
        "dependency",
        "documentation",
        "generated",
        "infrastructure",
        "production_code",
        "schema_migration",
        "security",
        "test",
        "vendored",
    ]);
    assert!(
        expected_categories.is_subset(&categories),
        "categories: {categories:#?}"
    );
    let unsupported_language = evidence.iter().find(|record| {
        record["payload"]["kind"] == "tracked_file"
            && record["payload"]["path"]["value"] == "native/unsupported.rs"
            && record["payload"]["language_status"] == "unsupported"
    });
    assert!(unsupported_language.is_some());
    let unavailable_feature = evidence.iter().find(|record| {
        record["payload"]["kind"] == "repository_feature"
            && record["payload"]["state"] == "unavailable"
            && record["payload"]["related_evidence_ids"]
                .as_array()
                .is_some_and(|ids| !ids.is_empty())
    });
    assert!(unavailable_feature.is_some());

    let limitations = manifest["limitations"]
        .as_array()
        .expect("manifest limitations");
    for code in [
        "attribute_resolution_unavailable",
        "project_scores_not_computed",
        "repository_code_not_executed",
    ] {
        assert!(limitations.iter().any(|item| item["code"] == code));
    }
    let text = String::from_utf8(first.stdout).expect("UTF-8 JSON");
    for forbidden in [
        fixture.repository.to_string_lossy().as_ref(),
        fixture.tripwire.to_string_lossy().as_ref(),
        SECRET_MARKER,
        "private-source-body",
        "foundation-fixture@example.invalid",
        "raw_diff",
        "assay_score",
        "person_score",
    ] {
        assert!(
            !text.contains(forbidden),
            "published forbidden value: {forbidden}"
        );
    }
}

#[test]
fn citation_audit_rejects_removed_nested_and_manifest_branches() {
    let fixture = FoundationFixture::build();
    let output = run_analysis(&fixture);
    assert_eq!(output.status.code(), Some(0));
    let bundle: Value = serde_json::from_slice(&output.stdout).expect("analysis bundle");

    let mut removed_source = bundle.clone();
    let classification = removed_source["evidence"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|record| record["payload"]["kind"] == "file_classification")
        .unwrap();
    classification["payload"]
        .as_object_mut()
        .unwrap()
        .remove("source_evidence_id");
    assert!(audit_bundle_citations(&removed_source).is_err());

    let mut emptied_feature = bundle.clone();
    let feature = emptied_feature["evidence"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|record| {
            record["payload"]["kind"] == "repository_feature"
                && !record["payload"]["related_evidence_ids"]
                    .as_array()
                    .unwrap()
                    .is_empty()
        })
        .unwrap();
    feature["payload"]["related_evidence_ids"] = Value::Array(Vec::new());
    assert!(audit_bundle_citations(&emptied_feature).is_err());

    let mut removed_limitation = bundle;
    removed_limitation["manifest"]["limitations"][0]
        .as_object_mut()
        .unwrap()
        .remove("affected_evidence_ids");
    assert!(audit_bundle_citations(&removed_limitation).is_err());
}
