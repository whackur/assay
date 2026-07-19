use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "assay",
    version,
    about = "Evidence-grounded repository analysis",
    color = clap::ColorChoice::Never
)]
pub struct Cli {
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
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
pub(crate) struct ProjectCommand {
    #[command(subcommand)]
    pub(crate) command: ProjectSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum ProjectSubcommand {
    Analyze(AnalyzeArgs),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum OutputFormat {
    Json,
}

/// Selectable evaluator IDs from the static registry (ADR 0012). The
/// deterministic default performs no AI evaluation; the AI evaluator IDs are
/// selectable so the interface is stable, but without an explicit
/// [`assay_local::ConsentGrant`] no external provider is ever constructed and
/// the evaluation section stays `disabled` with `user_consent_required`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum Evaluator {
    Deterministic,
    #[value(name = "openai-api-1")]
    OpenaiApi1,
    #[value(name = "codex-cli-1")]
    CodexCli1,
}

impl Evaluator {
    /// Returns the stable registry identifier for this selection.
    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::Deterministic => "deterministic",
            Self::OpenaiApi1 => "openai-api-1",
            Self::CodexCli1 => "codex-cli-1",
        }
    }
}

#[derive(Debug, Args)]
pub(crate) struct AnalyzeArgs {
    #[arg(default_value = ".")]
    pub(crate) repository: PathBuf,
    #[arg(long, default_value = "HEAD")]
    pub(crate) revision: String,
    #[arg(long, value_enum, default_value = "deterministic")]
    pub(crate) evaluator: Evaluator,
    #[arg(long, value_enum, default_value = "json")]
    pub(crate) format: OutputFormat,
    #[arg(long, default_value = "-")]
    pub(crate) output: PathBuf,
    #[arg(long)]
    pub(crate) no_color: bool,
    #[arg(long)]
    pub(crate) non_interactive: bool,
    /// Name of an environment variable holding a least-privilege GitHub PAT.
    /// The token value is never read into an argument, log, result, or record.
    #[arg(long, value_name = "VAR")]
    pub(crate) github_token_env: Option<String>,
    /// Append the analysis to an immutable local history directory.
    #[arg(long, value_name = "DIR")]
    pub(crate) record_history: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub(crate) struct ServeArgs {
    #[arg(long, value_name = "DIR")]
    pub(crate) history: PathBuf,
    #[arg(long, default_value_t = 7878)]
    pub(crate) port: u16,
    #[arg(long)]
    pub(crate) no_color: bool,
    /// Serve a single request then exit. Intended for smoke tests.
    #[arg(long, hide = true)]
    pub(crate) once: bool,
}

#[derive(Debug, Args)]
pub(crate) struct HistoryCommand {
    #[command(subcommand)]
    pub(crate) command: HistorySubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum HistorySubcommand {
    /// Soft-delete a record. Local-operator action.
    Delete(HistoryRecordArgs),
    /// Restore a soft-deleted record. Local-operator action.
    Restore(HistoryRecordArgs),
    /// Purge a record irrecoverably. Local-operator action.
    Purge(HistoryRecordArgs),
}

#[derive(Debug, Args)]
pub(crate) struct HistoryRecordArgs {
    pub(crate) id: String,
    #[arg(long, value_name = "DIR")]
    pub(crate) history: PathBuf,
    #[arg(long, default_value = "-")]
    pub(crate) output: PathBuf,
    #[arg(long)]
    pub(crate) no_color: bool,
}

#[derive(Debug, Args)]
pub(crate) struct CapabilitiesArgs {
    #[arg(long, value_enum, default_value = "json")]
    pub(crate) format: OutputFormat,
    #[arg(long, default_value = "-")]
    pub(crate) output: PathBuf,
    #[arg(long)]
    pub(crate) no_color: bool,
}
