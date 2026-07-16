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
use assay_local::{
    ConsentState, GithubTokenEnvVar, LocalAdministrator, LocalHistoryStore, LocalReport,
    LoopbackListener, run as serve_run, serve_next,
};
use assay_project_intelligence::{
    ClassifiedSnapshotFile, assemble_project_evidence, build_project_analysis,
    validate_project_bundle_consistency,
};
use clap::{Args, Parser, Subcommand, ValueEnum};
use jsonschema::{Draft, Resource, Validator};
use serde_json::{Value, json};
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
    /// Serve the local dashboard on the loopback interface only.
    Serve(ServeArgs),
    /// Administer immutable local analysis history.
    History(HistoryCommand),
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
    /// Name of an environment variable holding a least-privilege GitHub PAT.
    /// The token value is never read into an argument, log, result, or record.
    #[arg(long, value_name = "VAR")]
    github_token_env: Option<String>,
    /// Append the analysis to an immutable local history directory.
    #[arg(long, value_name = "DIR")]
    record_history: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct ServeArgs {
    #[arg(long, value_name = "DIR")]
    history: PathBuf,
    #[arg(long, default_value_t = 7878)]
    port: u16,
    #[arg(long)]
    no_color: bool,
    /// Serve a single request then exit. Intended for smoke tests.
    #[arg(long, hide = true)]
    once: bool,
}

#[derive(Debug, Args)]
struct HistoryCommand {
    #[command(subcommand)]
    command: HistorySubcommand,
}

#[derive(Debug, Subcommand)]
enum HistorySubcommand {
    /// Soft-delete a record. Local-operator action.
    Delete(HistoryRecordArgs),
    /// Restore a soft-deleted record. Local-operator action.
    Restore(HistoryRecordArgs),
    /// Purge a record irrecoverably. Local-operator action.
    Purge(HistoryRecordArgs),
}

#[derive(Debug, Args)]
struct HistoryRecordArgs {
    id: String,
    #[arg(long, value_name = "DIR")]
    history: PathBuf,
    #[arg(long, default_value = "-")]
    output: PathBuf,
    #[arg(long)]
    no_color: bool,
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
    match run(cli, stderr) {
        Ok(Outcome::Emit { bytes, destination }) => {
            match write_output(&bytes, &destination, stdout) {
                Ok(()) => 0,
                Err(code) => {
                    let _ = writeln!(stderr, "error: output_failed code={code}");
                    12
                }
            }
        }
        Ok(Outcome::Served) => 0,
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

enum Outcome {
    Emit {
        bytes: Vec<u8>,
        destination: PathBuf,
    },
    Served,
}

fn emit(bytes: Vec<u8>, destination: PathBuf) -> Outcome {
    Outcome::Emit { bytes, destination }
}

fn run(cli: Cli, stderr: &mut dyn Write) -> Result<Outcome, RunError> {
    match cli.command {
        Command::Capabilities(arguments) => {
            let value = capabilities();
            validate("capabilities", &value)?;
            Ok(emit(json_bytes(&value)?, arguments.output))
        }
        Command::Project(ProjectCommand {
            command: ProjectSubcommand::Analyze(arguments),
        }) => analyze(arguments),
        Command::Serve(arguments) => serve(arguments, stderr),
        Command::History(HistoryCommand { command }) => history(command),
    }
}

fn analyze(arguments: AnalyzeArgs) -> Result<Outcome, RunError> {
    let _delivery_contract = (
        arguments.evaluator,
        arguments.format,
        arguments.no_color,
        arguments.non_interactive,
    );
    // Validate the token variable *name* eagerly; the value is never read here.
    // An already-cloned local repository is analyzed without credentials.
    if let Some(name) = &arguments.github_token_env {
        GithubTokenEnvVar::parse(name).map_err(|_| RunError {
            exit_code: 2,
            message: "invalid_input field=github_token_env".into(),
        })?;
    }
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
    if let Some(directory) = &arguments.record_history {
        record_local_history(directory, value.clone(), &generated_at)?;
    }
    Ok(emit(json_bytes(&value)?, arguments.output))
}

fn record_local_history(
    directory: &Path,
    analysis: Value,
    generated_at: &str,
) -> Result<(), RunError> {
    let report = LocalReport::from_analysis(analysis, &ConsentState::default(), generated_at)
        .map_err(|_| RunError {
            exit_code: 13,
            message: "history_record_invalid".into(),
        })?;
    let store = LocalHistoryStore::open(directory).map_err(|_| history_write_error())?;
    store
        .append(report.to_value(), generated_at)
        .map_err(|_| history_write_error())?;
    Ok(())
}

fn history_write_error() -> RunError {
    RunError {
        exit_code: 13,
        message: "history_write_failed".into(),
    }
}

fn serve(arguments: ServeArgs, stderr: &mut dyn Write) -> Result<Outcome, RunError> {
    let _no_color = arguments.no_color;
    let store = LocalHistoryStore::open(&arguments.history).map_err(|_| history_write_error())?;
    let listener = LoopbackListener::bind(arguments.port).map_err(|_| RunError {
        exit_code: 14,
        message: "serve_bind_failed".into(),
    })?;
    if let Ok(address) = listener.local_addr() {
        let _ = writeln!(stderr, "listening on http://{address}");
    }
    let served = if arguments.once {
        serve_next(&listener, &store)
    } else {
        serve_run(&listener, &store)
    };
    served.map_err(|_| RunError {
        exit_code: 14,
        message: "serve_failed".into(),
    })?;
    Ok(Outcome::Served)
}

fn history(command: HistorySubcommand) -> Result<Outcome, RunError> {
    let (action, arguments) = match command {
        HistorySubcommand::Delete(arguments) => ("soft_delete", arguments),
        HistorySubcommand::Restore(arguments) => ("restore", arguments),
        HistorySubcommand::Purge(arguments) => ("purge", arguments),
    };
    let _no_color = arguments.no_color;
    let store = LocalHistoryStore::open(&arguments.history).map_err(|_| history_write_error())?;
    let operator = LocalAdministrator::assume_local_operator();
    let at = current_time()?;
    let result = match action {
        "soft_delete" => store.soft_delete(&arguments.id, &operator, &at),
        "restore" => store.restore(&arguments.id, &operator, &at),
        _ => store.purge(&arguments.id, &operator, &at),
    };
    result.map_err(|_| RunError {
        exit_code: 13,
        message: "history_operation_failed".into(),
    })?;
    let value = json!({
        "schema_version": "1.0.0",
        "action": action,
        "id": arguments.id,
        "status": "ok"
    });
    Ok(emit(json_bytes(&value)?, arguments.output))
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
        "commands": ["capabilities", "history", "project analyze", "serve"],
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
            { "id": "local_private_history", "status": "implemented" },
            { "id": "loopback_dashboard", "status": "implemented" },
            { "id": "project_scores", "status": "not_implemented" },
            { "id": "remote_private_fetch", "status": "not_implemented" },
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
