//! First public MVP integration verification.
//!
//! These tests cross real crate boundaries with fresh producer output rather
//! than re-checking a single crate. The `assay` binary analyzes a deterministic
//! fixture repository, its machine output is validated against the public
//! schemas, and the same evidence identifiers feed the AI evaluator
//! (`assay-ai-evaluator`), the shared domain judgment contract (`assay-domain`),
//! and the deterministic score compiler (`assay-project-intelligence`) through
//! to a schema-valid `project-evaluation`.
//!
//! The evidence-manifest-to-evaluation-bundle adapter and the compiler are not
//! wired into the CLI in the foundation milestone, so the bundle is assembled
//! here from identifiers the CLI actually emitted. The status handoff records
//! this as deliberately deferred wiring.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::str::FromStr;

use assay_ai_evaluator::{
    DeterministicFakeProvider, Evaluator, EvidenceBundle, EvidenceDescriptor, EvidenceKind,
    EvidenceScope, ExternalTransmission, QualitativeRubric, ValidatedJudgmentSet,
};
use assay_domain::{EvidenceId, EvidenceStatus, RepositorySource, RevisionId, RubricJudgmentSet};
use assay_project_intelligence::{
    CitedStatement, CompiledEvaluation, CompilerPolicy, EvaluatorDescriptor, EvaluatorProvider,
    PotentialContext, ProjectClassification, ProjectMaturity, ProjectType, ScoreCompilerInput,
    Visibility, validate_project_bundle_consistency,
};
use assay_test_fixtures::{RepositoryFixture, RepositoryScenario};
use jsonschema::{Draft, Resource, Validator};
use serde_json::Value;

const FIXED_TIME: &str = "2026-01-02T03:04:06Z";
const PRODUCED_EVALUATION: &str = "tests/integration/produced/project-evaluation.json";

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_assay")
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("assay-cli must remain under crates/")
        .to_path_buf()
}

// Runs `assay project analyze` on a fixture with a fixed clock and returns the
// machine output written to stdout.
fn analyze(repository: &std::path::Path) -> Vec<u8> {
    let output = Command::new(binary())
        .env_clear()
        .env("ASSAY_TEST_FIXED_TIME", FIXED_TIME)
        .arg("project")
        .arg("analyze")
        .arg(repository)
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
        .expect("analyze subprocess must start");
    assert!(
        output.status.success(),
        "analyze failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "analyze must not log to stderr");
    output.stdout
}

fn schema_validator(contract: &str) -> Validator {
    let schemas = repository_root().join("schemas");
    let mut root = None;
    let resources = std::fs::read_dir(&schemas)
        .expect("schema directory must be readable")
        .filter_map(|entry| {
            let path = entry.expect("schema entry must be readable").path();
            let file = path.join("v1.json");
            if !file.is_file() {
                return None;
            }
            let schema: Value = serde_json::from_str(
                &std::fs::read_to_string(&file).expect("schema must be readable"),
            )
            .expect("schema must parse");
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .expect("schema directory name")
                .to_owned();
            if name == contract {
                root = Some(schema.clone());
            }
            let id = schema["$id"]
                .as_str()
                .expect("schema must declare $id")
                .to_owned();
            Some((
                id,
                Resource::from_contents(schema).expect("schema resource must build"),
            ))
        })
        .collect::<Vec<_>>();
    let root = root.unwrap_or_else(|| panic!("unknown schema contract: {contract}"));
    jsonschema::options()
        .with_draft(Draft::Draft202012)
        .should_validate_formats(true)
        .with_resources(resources.into_iter())
        .build(&root)
        .expect("schema must build")
}

fn assert_valid(contract: &str, instance: &Value) {
    let validator = schema_validator(contract);
    let errors = validator
        .iter_errors(instance)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(
        errors.is_empty(),
        "{contract} rejected fresh producer output: {errors:#?}"
    );
}

fn evidence_id(bundle: &Value, kind: &str) -> EvidenceId {
    let raw = bundle["evidence"]
        .as_array()
        .expect("evidence must be an array")
        .iter()
        .find(|fact| fact["payload"]["kind"] == kind)
        .unwrap_or_else(|| panic!("fresh evidence must contain a {kind} fact"))["id"]
        .as_str()
        .expect("evidence id must be a string");
    EvidenceId::from_str(raw).expect("CLI evidence id must be a valid domain id")
}

fn descriptor(id: EvidenceId, kind: EvidenceKind, statement: &str) -> EvidenceDescriptor {
    EvidenceDescriptor::new(id, kind, statement).expect("bounded statement must be accepted")
}

// Builds the evaluation bundle the missing CLI adapter would build, keyed by
// real producer evidence identifiers.
fn evidence_bundle(ids: [EvidenceId; 3]) -> EvidenceBundle {
    let [claim, test, fact] = ids;
    EvidenceBundle::new(
        EvidenceScope::PublicOnly,
        ExternalTransmission::NotUsed,
        vec![
            descriptor(
                claim,
                EvidenceKind::DocumentationClaim,
                "The project documents a repository analysis workflow.",
            ),
            descriptor(
                test,
                EvidenceKind::Test,
                "A cited test exercises the documented workflow.",
            ),
            descriptor(
                fact,
                EvidenceKind::RepositoryFact,
                "The analyzed source revision is immutable.",
            ),
        ],
    )
    .expect("evidence bundle must be valid")
}

fn validated_judgments(bundle: &EvidenceBundle) -> ValidatedJudgmentSet {
    Evaluator::new(QualitativeRubric::project_v1())
        .evaluate(&DeterministicFakeProvider::valid(), bundle)
        .expect("the deterministic provider must produce a valid judgment set")
}

fn compile(judgments: RubricJudgmentSet, primary: EvidenceId) -> CompiledEvaluation {
    let classification = ProjectClassification::new(
        EvidenceStatus::Complete,
        Some(ProjectType::CliDeveloperTool),
        vec![ProjectType::LibrarySdkFramework],
        vec!["developer_tool".to_owned()],
        Some(ProjectMaturity::Prototype),
        0.76,
        vec![primary.clone()],
    )
    .expect("classification must be valid");
    let potential = PotentialContext::new(
        vec![
            CitedStatement::new(
                "Continued iteration is required before a numeric forecast.",
                vec![primary.clone()],
            )
            .expect("assumption must cite evidence"),
        ],
        vec![
            CitedStatement::new(
                "The current evidence is insufficient for a numeric forecast.",
                vec![primary.clone()],
            )
            .expect("counter-signal must cite evidence"),
        ],
    );
    ScoreCompilerInput::new(
        RepositorySource::hosted("github", "example-org", "sample-project")
            .expect("project source must be valid"),
        RevisionId::from_str("0123456789abcdef0123456789abcdef01234567").expect("revision"),
        EvaluatorDescriptor::new(
            "deterministic-project-evaluator-1",
            EvaluatorProvider::Deterministic,
            None,
            "project-rubric-1",
        )
        .expect("evaluator descriptor must be valid"),
        Visibility::PrivateLocal,
        classification,
        Vec::new(),
        Some(judgments),
        potential,
        CompilerPolicy::v1(),
    )
    .compile()
    .expect("score compiler must accept the mapped judgments")
}

#[test]
fn fresh_cli_output_is_schema_valid_bundle_and_byte_deterministic() {
    let fixture = RepositoryFixture::build(RepositoryScenario::TypeScriptProject)
        .expect("fixture must build");

    let first = analyze(fixture.path());
    let second = analyze(fixture.path());
    assert_eq!(
        first, second,
        "the analysis producer must be byte-deterministic under a fixed clock"
    );

    let bundle: Value = serde_json::from_slice(&first).expect("analysis must be JSON");
    assert_valid("project-analysis", &bundle);
    assert_valid("analysis-manifest", &bundle["manifest"]);
    for fact in bundle["evidence"].as_array().expect("evidence array") {
        assert_valid("project-evidence", fact);
    }
    validate_project_bundle_consistency(&bundle)
        .expect("fresh bundle must satisfy cross-component invariants");
}

#[test]
fn cli_evidence_flows_through_evaluator_domain_and_score_compiler() {
    let fixture = RepositoryFixture::build(RepositoryScenario::TypeScriptProject)
        .expect("fixture must build");
    let bundle: Value = serde_json::from_slice(&analyze(fixture.path())).expect("analysis JSON");

    let readme = evidence_id(&bundle, "repository_feature");
    let history = evidence_id(&bundle, "history_scope");
    let snapshot = evidence_id(&bundle, "repository_snapshot");
    let evaluation_ids = [readme.clone(), history, snapshot];

    let evidence_bundle = evidence_bundle(evaluation_ids.clone());
    let validated = validated_judgments(&evidence_bundle);
    let domain_set = validated
        .to_rubric_judgment_set()
        .expect("validated judgments must map onto the domain contract");

    // Every judgment citation is a real identifier the CLI produced.
    let produced: Vec<&EvidenceId> = evaluation_ids.iter().collect();
    for judgment in domain_set.judgments() {
        for citation in judgment.evidence_ids() {
            assert!(
                produced.contains(&citation),
                "a mapped citation must stay inside the fresh CLI evidence set"
            );
        }
    }

    let compiled = compile(domain_set, readme);
    let evaluation = compiled.to_machine_value();
    assert_valid("project-evaluation", &evaluation);
    assert_eq!(evaluation["evaluation_version"], "project-intelligence-1");
    assert_eq!(
        evaluation["compiler"]["judgment_bundle_hash"].as_str(),
        Some(evidence_bundle.content_hash()),
        "the compiled evaluation must record the evaluated bundle hash"
    );
}

#[test]
fn full_chain_evaluation_is_deterministic_and_matches_committed_fixture() {
    // Machine-independent identifiers keep the committed fixture stable across
    // machines while still exercising the evaluator -> domain -> compiler chain.
    let ids = [
        EvidenceId::from_str("evidence:readme:claim-1").unwrap(),
        EvidenceId::from_str("evidence:test:integration-1").unwrap(),
        EvidenceId::from_str("evidence:repository:snapshot").unwrap(),
    ];
    let build = || {
        let bundle = evidence_bundle(ids.clone());
        let judgments = validated_judgments(&bundle)
            .to_rubric_judgment_set()
            .expect("judgments must map onto the domain contract");
        compile(judgments, ids[0].clone()).to_machine_value()
    };

    let evaluation = build();
    assert_valid("project-evaluation", &evaluation);

    let mut serialized = serde_json::to_vec_pretty(&evaluation).expect("evaluation must serialize");
    serialized.push(b'\n');
    let again = {
        let mut bytes = serde_json::to_vec_pretty(&build()).unwrap();
        bytes.push(b'\n');
        bytes
    };
    assert_eq!(
        serialized, again,
        "the full chain must be byte-deterministic"
    );

    let path = repository_root().join(PRODUCED_EVALUATION);
    if std::env::var_os("ASSAY_BLESS_PRODUCED").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &serialized).unwrap();
    }
    let committed = std::fs::read(&path).unwrap_or_else(|error| {
        panic!(
            "missing produced fixture {}: {error}; regenerate with ASSAY_BLESS_PRODUCED=1",
            path.display()
        )
    });
    assert_eq!(
        committed, serialized,
        "committed producer fixture is stale; regenerate with ASSAY_BLESS_PRODUCED=1"
    );
}

// Spawns `serve --once`, issues one GET, and returns the raw HTTP response.
fn serve_once_get(history: &std::path::Path, path: &str) -> String {
    let mut command = Command::new(binary());
    command.env_clear().env("ASSAY_TEST_FIXED_TIME", FIXED_TIME);
    // Windows sockets fail to initialize without `SystemRoot`, so preserve it
    // after clearing the environment. It carries no repository-derived input.
    #[cfg(windows)]
    if let Some(root) = std::env::var_os("SystemRoot") {
        command.env("SystemRoot", root);
    }
    let mut child = command
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
fn record_history_round_trips_the_local_report_contract_over_serve() {
    let fixture = RepositoryFixture::build(RepositoryScenario::TypeScriptProject)
        .expect("fixture must build");
    let history = tempfile::TempDir::new().unwrap();

    let recorded = Command::new(binary())
        .env_clear()
        .env("ASSAY_TEST_FIXED_TIME", FIXED_TIME)
        .arg("project")
        .arg("analyze")
        .arg(fixture.path())
        .args(["--evaluator", "deterministic", "--output", "-"])
        .arg("--record-history")
        .arg(history.path())
        .output()
        .expect("analyze subprocess must start");
    assert!(recorded.status.success());

    let response = serve_once_get(history.path(), "/api/history/rec-000001");
    assert!(
        response.starts_with("HTTP/1.1 200 OK"),
        "response: {response}"
    );
    let body = response
        .split("\r\n\r\n")
        .nth(1)
        .expect("response must have a body");
    let report: Value = serde_json::from_str(body.trim()).expect("served report must be JSON");

    assert_eq!(report["schema_version"], "1.0.0");
    assert_eq!(report["visibility"], "private_local");
    assert_eq!(report["privacy"]["visibility"], "private_local");
    assert_eq!(report["privacy"]["catalog_eligible"], false);
    assert_eq!(
        report["privacy"]["external_transmission"],
        "consent_required"
    );
    assert_eq!(report["sections"]["ai_evaluation"]["state"], "disabled");
    assert_eq!(
        report["sections"]["competitor_discovery"]["state"],
        "disabled"
    );
    // The immutable analysis the CLI produced is embedded verbatim.
    assert_eq!(report["analysis"]["schema_version"], "1.0.0");
}
