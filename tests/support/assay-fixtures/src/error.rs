//! Redacted failure information for fixture creation.

use std::{fmt, io};

/// Redacted failure information for fixture creation.
pub struct FixtureBuildError {
    pub(crate) stage: BuildStage,
    pub(crate) reason: FailureReason,
}

impl FixtureBuildError {
    pub(crate) fn io(stage: BuildStage, error: io::Error) -> Self {
        Self {
            stage,
            reason: FailureReason::Io(error.kind()),
        }
    }

    pub(crate) fn git(stage: BuildStage, output: &std::process::Output) -> Self {
        Self {
            stage,
            reason: FailureReason::GitExit(output.status.code()),
        }
    }

    pub(crate) fn invalid_utf8(stage: BuildStage) -> Self {
        Self {
            stage,
            reason: FailureReason::InvalidUtf8,
        }
    }
}

impl fmt::Debug for FixtureBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FixtureBuildError")
            .field("stage", &self.stage)
            .field("reason", &self.reason)
            .finish()
    }
}

impl fmt::Display for FixtureBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "repository fixture build failed during {} ({})",
            self.stage, self.reason
        )
    }
}

impl std::error::Error for FixtureBuildError {}

#[derive(Clone, Copy, Debug)]
pub(crate) enum BuildStage {
    CreateTemporaryDirectory,
    CreateGitConfiguration,
    CreateRepositoryDirectory,
    InitializeGitRepository,
    ConfigureGitRepository,
    WriteFixtureFile,
    RemoveFixtureFile,
    StageFixtureFiles,
    NormalizeFileMode,
    CreateCommit,
    ResolveCommit,
}

impl fmt::Display for BuildStage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::CreateTemporaryDirectory => "temporary directory creation",
            Self::CreateGitConfiguration => "Git configuration isolation",
            Self::CreateRepositoryDirectory => "repository directory creation",
            Self::InitializeGitRepository => "Git repository initialization",
            Self::ConfigureGitRepository => "Git repository configuration",
            Self::WriteFixtureFile => "fixture file creation",
            Self::RemoveFixtureFile => "fixture file removal",
            Self::StageFixtureFiles => "fixture staging",
            Self::NormalizeFileMode => "file mode normalization",
            Self::CreateCommit => "commit creation",
            Self::ResolveCommit => "commit resolution",
        };
        formatter.write_str(name)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum FailureReason {
    Io(io::ErrorKind),
    GitExit(Option<i32>),
    InvalidUtf8,
}

impl fmt::Display for FailureReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(kind) => write!(formatter, "I/O error: {kind:?}"),
            Self::GitExit(Some(code)) => write!(formatter, "Git exit code {code}"),
            Self::GitExit(None) => formatter.write_str("Git terminated without an exit code"),
            Self::InvalidUtf8 => formatter.write_str("Git returned non-UTF-8 object metadata"),
        }
    }
}
