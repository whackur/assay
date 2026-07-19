use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
};

use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

use crate::{
    CollectionError, CollectionErrorKind, CollectionLimits, CollectionStage, GitObjectFormat,
    GitObjectId, process::GitProcessRunner,
};

use super::error::incompatible_git;
use super::parse::{parse_version, single_line};

/// Read-only installed-Git adapter selected by ADR 0002.
pub struct GitCliAdapter {
    pub(crate) runner: GitProcessRunner,
    pub(crate) limits: CollectionLimits,
    pub(crate) git_version: String,
}

impl GitCliAdapter {
    /// Probes one deployment-trusted absolute executable exactly once.
    ///
    /// The probe requires Git 2.47 or newer and the global
    /// `--no-lazy-fetch` option. Repository content never selects this path.
    pub fn from_trusted_executable(
        executable: PathBuf,
        limits: CollectionLimits,
    ) -> Result<Self, CollectionError> {
        if !limits.is_valid() {
            return Err(CollectionError::new(
                CollectionStage::ConfigureAdapter,
                CollectionErrorKind::InvalidLimits,
            ));
        }
        if !executable.is_absolute() {
            return Err(CollectionError::new(
                CollectionStage::ConfigureAdapter,
                CollectionErrorKind::UntrustedExecutable,
            ));
        }
        let runner = GitProcessRunner::new(executable, limits);
        let output = runner
            .run(
                None,
                CollectionStage::ProbeCapabilities,
                &[OsStr::new("version")],
                256,
            )
            .map_err(|error| {
                if error.kind() == CollectionErrorKind::NonZeroExit {
                    incompatible_git()
                } else {
                    error
                }
            })?;
        let version = parse_version(&output)?;
        Ok(Self {
            runner,
            limits,
            git_version: version,
        })
    }

    pub(crate) fn resolve_revision(
        &self,
        repository: &Path,
        revision: &OsStr,
        format: GitObjectFormat,
    ) -> Result<GitObjectId, CollectionError> {
        let mut peeled = revision.to_os_string();
        peeled.push("^{commit}");
        self.resolve_object(
            repository,
            &peeled,
            CollectionStage::ResolveRevision,
            format,
        )
    }

    pub(crate) fn resolve_tree(
        &self,
        repository: &Path,
        revision: &GitObjectId,
        format: GitObjectFormat,
    ) -> Result<GitObjectId, CollectionError> {
        let mut peeled = OsString::from(revision.as_str());
        peeled.push("^{tree}");
        self.resolve_object(repository, &peeled, CollectionStage::ResolveTree, format)
    }

    pub(crate) fn resolve_object(
        &self,
        repository: &Path,
        object: &OsStr,
        stage: CollectionStage,
        format: GitObjectFormat,
    ) -> Result<GitObjectId, CollectionError> {
        let output = self.runner.run(
            Some(repository),
            stage,
            &[
                OsStr::new("rev-parse"),
                OsStr::new("--verify"),
                OsStr::new("--end-of-options"),
                object,
            ],
            128,
        )?;
        GitObjectId::parse(single_line(&output, stage)?, stage, format)
    }

    pub(crate) fn commit_time(
        &self,
        repository: &Path,
        revision: &GitObjectId,
    ) -> Result<String, CollectionError> {
        let output = self.runner.run(
            Some(repository),
            CollectionStage::ReadCommitTime,
            &[
                OsStr::new("show"),
                OsStr::new("--no-patch"),
                OsStr::new("--format=%cI"),
                OsStr::new("--end-of-options"),
                OsStr::new(revision.as_str()),
            ],
            64,
        )?;
        let value = single_line(&output, CollectionStage::ReadCommitTime)?;
        let value = std::str::from_utf8(value).map_err(|_| {
            CollectionError::new(
                CollectionStage::ReadCommitTime,
                CollectionErrorKind::MalformedOutput,
            )
        })?;
        let time = OffsetDateTime::parse(value, &Rfc3339).map_err(|_| {
            CollectionError::new(
                CollectionStage::ReadCommitTime,
                CollectionErrorKind::MalformedOutput,
            )
        })?;
        time.to_offset(UtcOffset::UTC)
            .format(&Rfc3339)
            .map_err(|_| {
                CollectionError::new(
                    CollectionStage::ReadCommitTime,
                    CollectionErrorKind::MalformedOutput,
                )
            })
    }
}

impl std::fmt::Debug for GitCliAdapter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GitCliAdapter")
            .field("executable", &"<trusted-executable>")
            .field("limits", &self.limits)
            .field("git_version", &self.git_version)
            .finish()
    }
}
