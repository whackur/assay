//! Thin command-line delivery boundary for Assay.

#![forbid(unsafe_code)]

use std::{
    ffi::OsStr,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use assay_classifier::{BuiltInPolicy, LinguistAttributeFacts};
use assay_git::{
    CollectionError, CollectionErrorKind, CollectionLimits, GitCliAdapter, RepositorySnapshotPort,
    SnapshotRequest,
};
use assay_project_intelligence::{
    ClassifiedSnapshotFile, assemble_project_evidence, build_project_analysis,
};
use clap::{Args, Parser, Subcommand, ValueEnum};
use jsonschema::{Draft, Resource, Validator};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

pub const PACKAGE_NAME: &str = env!("CARGO_PKG_NAME");

#[derive(Debug, Parser)]
#[command(
    name = "assay",
    version,
    about = "Evidence-grounded repository analysis",
    color = clap::ColorChoice::Never
)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Analyze project-level repository evidence without computing scores.
    Project(ProjectCommand),
    /// Report only capabilities implemented by this binary.
    Capabilities(CapabilitiesArgs),
}

#[derive(Debug, Args)]
struct ProjectCommand {
    #[command(subcommand)]
    command: ProjectSubcommand,
}

#[derive(Debug, Subcommand)]
enum ProjectSubcommand {
    Analyze(AnalyzeArgs),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum OutputFormat {
    Json,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum Evaluator {
    Deterministic,
}

#[derive(Debug, Args)]
struct AnalyzeArgs {
    #[arg(default_value = ".")]
    repository: PathBuf,
    #[arg(long, default_value = "HEAD")]
    revision: String,
    #[arg(long, value_enum, default_value = "deterministic")]
    evaluator: Evaluator,
    #[arg(long, value_enum, default_value = "json")]
    format: OutputFormat,
    #[arg(long, default_value = "-")]
    output: PathBuf,
    #[arg(long)]
    no_color: bool,
    #[arg(long)]
    non_interactive: bool,
}

#[derive(Debug, Args)]
struct CapabilitiesArgs {
    #[arg(long, value_enum, default_value = "json")]
    format: OutputFormat,
    #[arg(long, default_value = "-")]
    output: PathBuf,
    #[arg(long)]
    no_color: bool,
}

/// Executes parsed CLI delivery with explicit output streams.
pub fn execute(cli: Cli, stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    match run(cli) {
        Ok((output, destination)) => match write_output(&output, &destination, stdout) {
            Ok(()) => 0,
            Err(code) => {
                let _ = writeln!(stderr, "error: output_failed code={code}");
                12
            }
        },
        Err(error) => {
            let _ = writeln!(stderr, "error: {}", error.message);
            error.exit_code
        }
    }
}

struct RunError {
    exit_code: i32,
    message: String,
}

fn run(cli: Cli) -> Result<(Vec<u8>, PathBuf), RunError> {
    match cli.command {
        Command::Capabilities(arguments) => {
            let value = capabilities();
            validate("capabilities", &value)?;
            Ok((json_bytes(&value)?, arguments.output))
        }
        Command::Project(ProjectCommand {
            command: ProjectSubcommand::Analyze(arguments),
        }) => analyze(arguments),
    }
}

fn analyze(arguments: AnalyzeArgs) -> Result<(Vec<u8>, PathBuf), RunError> {
    let _delivery_contract = (
        arguments.evaluator,
        arguments.format,
        arguments.no_color,
        arguments.non_interactive,
    );
    let git = trusted_git().ok_or_else(|| RunError {
        exit_code: 10,
        message: "collection_failed stage=configure_adapter kind=executable_missing".into(),
    })?;
    if !arguments.repository.exists() {
        return Err(RunError {
            exit_code: 4,
            message: "source_not_found".into(),
        });
    }
    let adapter = GitCliAdapter::from_trusted_executable(git, collection_limits()?)
        .map_err(collection_error)?;
    let identity = adapter
        .derive_local_repository_source(&arguments.repository, OsStr::new(&arguments.revision))
        .map_err(collection_or_not_found)?;
    let snapshot = adapter
        .collect(SnapshotRequest::new(
            &arguments.repository,
            identity.source().clone(),
            OsStr::new(identity.revision().as_str()),
        ))
        .map_err(collection_or_not_found)?;
    let classifications = snapshot
        .entries()
        .iter()
        .map(|entry| {
            ClassifiedSnapshotFile::classify(
                &snapshot,
                entry,
                LinguistAttributeFacts::unavailable(),
                &BuiltInPolicy::V1,
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| RunError {
            exit_code: 11,
            message: "analysis_failed stage=file_classification".into(),
        })?;
    let evidence = assemble_project_evidence(&snapshot, classifications).map_err(|_| RunError {
        exit_code: 11,
        message: "analysis_failed stage=evidence_assembly".into(),
    })?;
    let generated_at = current_time()?;
    let value =
        build_project_analysis(&snapshot, &evidence, &generated_at).map_err(|_| RunError {
            exit_code: 11,
            message: "analysis_failed stage=machine_mapping".into(),
        })?;
    validate("project-analysis", &value)?;
    validate_project_bundle_consistency(&value).map_err(|_| bundle_error())?;
    Ok((json_bytes(&value)?, arguments.output))
}

fn collection_limits() -> Result<CollectionLimits, RunError> {
    let mut limits = CollectionLimits::default();
    if cfg!(debug_assertions) {
        if let Some(value) = std::env::var_os("ASSAY_TEST_MAX_OBJECT_BYTES") {
            limits.max_object_bytes = value
                .to_str()
                .and_then(|v| v.parse().ok())
                .filter(|v| *v > 0)
                .ok_or_else(invalid_test_limit)?;
        }
        if let Some(value) = std::env::var_os("ASSAY_TEST_MAX_HISTORY_COMMITS") {
            limits.max_history_commits = value
                .to_str()
                .and_then(|v| v.parse().ok())
                .filter(|v| *v > 0)
                .ok_or_else(invalid_test_limit)?;
        }
    }
    Ok(limits)
}

fn invalid_test_limit() -> RunError {
    RunError {
        exit_code: 2,
        message: "invalid_input field=collection_limit".into(),
    }
}

/// Enforces cross-component invariants that JSON Schema cannot express.
pub fn validate_project_bundle_consistency(value: &Value) -> Result<(), &'static str> {
    let manifest = value.get("manifest").ok_or("missing_manifest")?;
    let evidence = value
        .get("evidence")
        .and_then(Value::as_array)
        .ok_or("missing_evidence")?;
    let source = &manifest["source_snapshot"]["source"];
    let revision = &manifest["source_snapshot"]["revision"];
    let mut previous: Option<&str> = None;
    for fact in evidence {
        if &fact["repository"] != source {
            return Err("source_mismatch");
        }
        if let Some(provenance) = fact.get("provenance")
            && !provenance["repository_revision"].is_null()
            && &provenance["repository_revision"] != revision
        {
            return Err("revision_mismatch");
        }
        let id = fact["id"].as_str().ok_or("missing_evidence_id")?;
        if previous.is_some_and(|prior| prior >= id) {
            return Err("evidence_order");
        }
        previous = Some(id);
    }
    let artifacts = manifest["artifacts"].as_array().ok_or("missing_artifact")?;
    let mut matching = artifacts
        .iter()
        .filter(|item| item["role"] == "project_evidence");
    let artifact = matching.next().ok_or("missing_artifact")?;
    if matching.next().is_some() {
        return Err("duplicate_artifact");
    }
    if artifact["status"] != manifest["status"] {
        return Err("artifact_status");
    }
    if artifact["record_count"].as_u64() != Some(evidence.len() as u64) {
        return Err("artifact_count");
    }
    let canonical = serde_json::to_vec(evidence).map_err(|_| "artifact_serialization")?;
    let expected = format!("sha256:{:x}", Sha256::digest(canonical));
    if artifact["content_hash"].as_str() != Some(expected.as_str()) {
        return Err("artifact_hash");
    }
    for source in manifest["data_sources"]
        .as_array()
        .ok_or("missing_data_sources")?
    {
        if !source["revision"].is_null() && source["revision"] != *revision {
            return Err("data_source_revision");
        }
    }
    Ok(())
}

fn bundle_error() -> RunError {
    RunError {
        exit_code: 12,
        message: "schema_validation_failed invariant=project_bundle".into(),
    }
}

fn capabilities() -> Value {
    json!({
        "schema_version": "1.0.0",
        "tool": { "name": "assay", "version": env!("CARGO_PKG_VERSION") },
        "commands": ["capabilities", "project analyze"],
        "formats": ["json"],
        "schemas": [
            { "name": "analysis-manifest", "version": "1.0.0" },
            { "name": "capabilities", "version": "1.0.0" },
            { "name": "project-analysis", "version": "1.0.0" },
            { "name": "project-evidence", "version": "1.0.0" }
        ],
        "languages": ["javascript", "python", "tsx", "typescript"],
        "features": [
            { "id": "ai_evaluation", "status": "not_implemented" },
            { "id": "attribute_resolution", "status": "not_implemented" },
            { "id": "file_classification", "status": "implemented" },
            { "id": "github_collection", "status": "not_implemented" },
            { "id": "local_git_snapshot", "status": "implemented" },
            { "id": "project_scores", "status": "not_implemented" },
            { "id": "repository_code_execution", "status": "prohibited" },
            { "id": "semantic_diff", "status": "not_implemented" }
        ]
    })
}

fn current_time() -> Result<String, RunError> {
    if cfg!(debug_assertions)
        && let Some(value) = std::env::var_os("ASSAY_TEST_FIXED_TIME")
    {
        let value = value.into_string().map_err(|_| invalid_clock())?;
        let parsed = OffsetDateTime::parse(&value, &Rfc3339).map_err(|_| invalid_clock())?;
        return parsed
            .to_offset(UtcOffset::UTC)
            .format(&Rfc3339)
            .map_err(|_| invalid_clock());
    }
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| invalid_clock())
}

fn invalid_clock() -> RunError {
    RunError {
        exit_code: 12,
        message: "schema_validation_failed field=generated_at".into(),
    }
}

fn trusted_git() -> Option<PathBuf> {
    ["/usr/bin/git", "/usr/local/bin/git"]
        .into_iter()
        .map(PathBuf::from)
        .find(|path| path.is_file())
}

fn collection_or_not_found(error: CollectionError) -> RunError {
    if error.kind() == CollectionErrorKind::NonZeroExit
        && error.stage() == assay_git::CollectionStage::ResolveRevision
    {
        RunError {
            exit_code: 4,
            message: "source_or_revision_not_found".into(),
        }
    } else {
        collection_error(error)
    }
}

fn collection_error(error: CollectionError) -> RunError {
    RunError {
        exit_code: 10,
        message: format!(
            "collection_failed stage={} kind={}",
            debug_code(error.stage()),
            debug_code(error.kind())
        ),
    }
}

fn debug_code(value: impl std::fmt::Debug) -> String {
    let input = format!("{value:?}");
    let mut output = String::new();
    for (index, character) in input.chars().enumerate() {
        if character.is_ascii_uppercase() && index > 0 {
            output.push('_');
        }
        output.push(character.to_ascii_lowercase());
    }
    output
}

fn json_bytes(value: &Value) -> Result<Vec<u8>, RunError> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|_| RunError {
        exit_code: 12,
        message: "output_serialization_failed".into(),
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn write_output(
    bytes: &[u8],
    destination: &Path,
    stdout: &mut dyn Write,
) -> Result<(), &'static str> {
    if destination == Path::new("-") {
        stdout.write_all(bytes).map_err(|_| "stdout_write")?;
        stdout.flush().map_err(|_| "stdout_flush")?;
        return Ok(());
    }
    if fs::symlink_metadata(destination).is_ok() {
        return Err("destination_exists");
    }
    let parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temporary = NamedTempFile::new_in(parent).map_err(|_| "temporary_create")?;
    temporary.write_all(bytes).map_err(|_| "temporary_write")?;
    temporary
        .as_file_mut()
        .sync_all()
        .map_err(|_| "temporary_sync")?;
    temporary
        .persist_noclobber(destination)
        .map_err(|_| "atomic_persist")?;
    Ok(())
}

fn validate(contract: &str, value: &Value) -> Result<(), RunError> {
    let validator = schema_validator(contract).map_err(|_| RunError {
        exit_code: 12,
        message: "schema_configuration_failed".into(),
    })?;
    if validator.is_valid(value) {
        Ok(())
    } else {
        Err(RunError {
            exit_code: 12,
            message: "schema_validation_failed".into(),
        })
    }
}

fn schema_validator(contract: &str) -> Result<Validator, ()> {
    let schemas = [
        (
            "analysis-manifest",
            include_str!("../../../schemas/analysis-manifest/v1.json"),
        ),
        (
            "capabilities",
            include_str!("../../../schemas/capabilities/v1.json"),
        ),
        (
            "project-analysis",
            include_str!("../../../schemas/project-analysis/v1.json"),
        ),
        (
            "project-evidence",
            include_str!("../../../schemas/project-evidence/v1.json"),
        ),
    ];
    let mut parsed = BTreeSchemas::default();
    for (name, text) in schemas {
        parsed.insert(name, serde_json::from_str(text).map_err(|_| ())?);
    }
    let resources = parsed
        .values()
        .map(|schema| {
            let id = schema["$id"].as_str().ok_or(())?.to_owned();
            let resource = Resource::from_contents(schema.clone()).map_err(|_| ())?;
            Ok((id, resource))
        })
        .collect::<Result<Vec<_>, ()>>()?;
    let root = parsed.get(contract).ok_or(())?;
    jsonschema::options()
        .with_draft(Draft::Draft202012)
        .should_validate_formats(true)
        .with_resources(resources.into_iter())
        .build(root)
        .map_err(|_| ())
}

type BTreeSchemas = std::collections::BTreeMap<&'static str, Value>;
