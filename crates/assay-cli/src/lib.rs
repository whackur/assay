//! Thin command-line delivery boundary for Assay.

#![forbid(unsafe_code)]

pub mod evaluators;

use std::{
    ffi::{OsStr, OsString},
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

/// Selectable evaluator IDs from the static registry (ADR 0012). The
/// deterministic default performs no AI evaluation; the AI evaluator IDs are
/// selectable so the interface is stable, but without an explicit
/// [`assay_local::ConsentGrant`] no external provider is ever constructed and
/// the evaluation section stays `disabled` with `user_consent_required`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum Evaluator {
    Deterministic,
    #[value(name = "openai-api-1")]
    OpenaiApi1,
    #[value(name = "codex-cli-1")]
    CodexCli1,
}

impl Evaluator {
    /// Returns the stable registry identifier for this selection.
    const fn id(self) -> &'static str {
        match self {
            Self::Deterministic => "deterministic",
            Self::OpenaiApi1 => "openai-api-1",
            Self::CodexCli1 => "codex-cli-1",
        }
    }
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
        arguments.format,
        arguments.no_color,
        arguments.non_interactive,
    );
    // Consent gating runs before provider construction (ADR 0012). The local
    // slice exposes no consent-granting surface yet, so no matching
    // `ConsentGrant` can exist, no external provider is ever constructed, and
    // deterministic evidence is returned for every evaluator selection; the
    // recorded report keeps its `ai_evaluation` section `disabled` with
    // `user_consent_required`.
    let consent = evaluation_consent(arguments.evaluator);
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
        record_local_history(directory, value.clone(), &consent, &generated_at)?;
    }
    Ok(emit(json_bytes(&value)?, arguments.output))
}

/// Returns the consent posture governing one analysis run. Every selectable
/// evaluator ID starts from the no-grant default: `deterministic` performs no
/// AI evaluation, and the AI evaluator IDs (see
/// [`evaluators::EVALUATOR_REGISTRY`]) require an explicit informed grant
/// that no local surface can produce yet, so they stay consent-gated.
fn evaluation_consent(evaluator: Evaluator) -> ConsentState {
    let _selected = evaluator.id();
    ConsentState::default()
}

fn record_local_history(
    directory: &Path,
    analysis: Value,
    consent: &ConsentState,
    generated_at: &str,
) -> Result<(), RunError> {
    let report =
        LocalReport::from_analysis(analysis, consent, generated_at).map_err(|_| RunError {
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
            ai_evaluation_capability(),
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

/// Reports the `ai_evaluation` feature honestly from the static evaluator
/// registry (ADR 0012): every registered AI evaluator ID is listed with its
/// family and its per-binary status, and the feature claims `implemented`
/// only when at least one AI evaluator can actually run end to end. The
/// deterministic default performs no AI evaluation, so it never appears here.
fn ai_evaluation_capability() -> Value {
    let evaluators = evaluators::EVALUATOR_REGISTRY
        .iter()
        .filter(|descriptor| descriptor.family() != evaluators::EvaluatorFamily::Deterministic)
        .map(|descriptor| {
            json!({
                "id": descriptor.id(),
                "family": descriptor.family().code(),
                "status": if descriptor.is_implemented() { "implemented" } else { "not_implemented" }
            })
        })
        .collect::<Vec<_>>();
    let implemented = evaluators
        .iter()
        .any(|evaluator| evaluator["status"] == "implemented");
    json!({
        "id": "ai_evaluation",
        "status": if implemented { "implemented" } else { "not_implemented" },
        "evaluators": evaluators
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

/// Environment variable that names one trusted, absolute Git executable.
///
/// This is a trusted deployment or startup configuration input per ADR 0002
/// rule 1. It is never derived from repository content and lets operators use
/// a non-default install location on any platform.
const GIT_EXECUTABLE_ENV: &str = "ASSAY_GIT_EXECUTABLE";

/// Resolves the Git executable from trusted deployment configuration or a
/// trusted startup environment, never from repository content (ADR 0002).
fn trusted_git() -> Option<PathBuf> {
    resolve_trusted_git(std::env::var_os(GIT_EXECUTABLE_ENV))
}

/// Pure resolution used by [`trusted_git`], split out so the precedence and
/// absolute-path contract can be tested without mutating the process
/// environment. An explicit override is authoritative; the adapter still
/// probes it and reports an explicit error if it is not a compatible Git.
fn resolve_trusted_git(override_value: Option<OsString>) -> Option<PathBuf> {
    if let Some(value) = override_value
        && !value.is_empty()
    {
        return Some(PathBuf::from(value));
    }
    default_git_candidates()
        .into_iter()
        .find(|path| path.is_file())
}

/// Well-known absolute install locations for a supported Git on Unix.
#[cfg(unix)]
fn default_git_candidates() -> Vec<PathBuf> {
    ["/usr/bin/git", "/usr/local/bin/git"]
        .into_iter()
        .map(PathBuf::from)
        .collect()
}

/// Well-known absolute install locations for Git for Windows, derived from the
/// trusted `Program Files` startup environment with fixed fallbacks. Custom
/// installs are supported through [`GIT_EXECUTABLE_ENV`].
#[cfg(windows)]
fn default_git_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    for key in ["ProgramW6432", "ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(base) = std::env::var_os(key)
            && !base.is_empty()
        {
            let base = PathBuf::from(base);
            candidates.push(base.join(r"Git\cmd\git.exe"));
            candidates.push(base.join(r"Git\bin\git.exe"));
        }
    }
    candidates.push(PathBuf::from(r"C:\Program Files\Git\cmd\git.exe"));
    candidates.push(PathBuf::from(r"C:\Program Files\Git\bin\git.exe"));
    candidates
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_override_is_authoritative_over_defaults() {
        // Use the running test binary as a stand-in absolute executable so the
        // assertion holds on every platform without a real Git install.
        let executable = std::env::current_exe().expect("test binary path");
        let resolved = resolve_trusted_git(Some(executable.clone().into_os_string()));
        assert_eq!(resolved, Some(executable));
    }

    #[test]
    fn empty_override_falls_back_to_platform_defaults() {
        assert_eq!(
            resolve_trusted_git(Some(OsString::new())),
            resolve_trusted_git(None)
        );
    }

    #[test]
    fn selectable_evaluators_match_the_static_registry_exactly() {
        use clap::ValueEnum;

        let selectable = Evaluator::value_variants()
            .iter()
            .map(|evaluator| evaluator.id())
            .collect::<Vec<_>>();
        let registered = evaluators::EVALUATOR_REGISTRY
            .iter()
            .map(evaluators::EvaluatorDescriptor::id)
            .collect::<Vec<_>>();
        assert_eq!(selectable, registered);
        // The clap value names are the stable registry IDs themselves.
        for evaluator in Evaluator::value_variants() {
            let rendered = evaluator
                .to_possible_value()
                .expect("every evaluator is selectable")
                .get_name()
                .to_owned();
            assert_eq!(rendered, evaluator.id());
        }
    }

    #[test]
    fn ai_evaluation_capability_never_claims_an_unrunnable_evaluator() {
        let feature = ai_evaluation_capability();
        assert_eq!(feature["id"], "ai_evaluation");
        let evaluators = feature["evaluators"].as_array().unwrap();
        assert!(!evaluators.is_empty());
        // The feature may claim implemented only when some AI evaluator can
        // actually run end to end through this binary.
        let any_implemented = evaluators
            .iter()
            .any(|evaluator| evaluator["status"] == "implemented");
        assert_eq!(
            feature["status"] == "implemented",
            any_implemented,
            "feature status must derive from the per-evaluator statuses"
        );
        // The deterministic selection performs no AI evaluation.
        assert!(
            evaluators
                .iter()
                .all(|evaluator| evaluator["id"] != "deterministic")
        );
    }

    #[test]
    fn default_candidates_are_absolute_paths() {
        // The adapter rejects any non-absolute executable as untrusted, so every
        // default candidate must be absolute (ADR 0002 rule 1).
        let candidates = default_git_candidates();
        assert!(!candidates.is_empty());
        assert!(candidates.iter().all(|path| path.is_absolute()));
    }
}
