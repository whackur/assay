//! Deterministic temporary Git repositories for Assay integration tests.
//!
//! This crate is test support only. It creates synthetic repository histories
//! without installing, importing, building, testing, or executing their files.

#![forbid(unsafe_code)]

mod error;
mod git;
mod runner;
mod scenario;
mod spec;

pub use error::FixtureBuildError;
pub use git::{GIT_EXECUTABLE_ENV, trusted_git_executable};
pub use scenario::RepositoryScenario;

use std::{
    collections::BTreeSet,
    fmt, fs,
    path::{Path, PathBuf},
};

use tempfile::TempDir;

use error::BuildStage;
use runner::{
    GitRunner, apply_commit_files, configure_repository, create_commit, resolve_head, stage_files,
};
use scenario::RepositoryScenario as Scenario;

/// Builder for one isolated deterministic repository fixture.
pub struct RepositoryFixtureBuilder {
    scenario: Scenario,
    git_program: PathBuf,
    command_environment: Vec<(std::ffi::OsString, std::ffi::OsString)>,
}

impl RepositoryFixtureBuilder {
    /// Creates a builder that invokes `git` directly without a shell.
    pub fn new(scenario: RepositoryScenario) -> Self {
        Self {
            scenario,
            git_program: PathBuf::from("git"),
            command_environment: Vec::new(),
        }
    }

    /// Selects a Git executable path, primarily for hermetic failure tests.
    pub fn git_program(mut self, program: impl Into<PathBuf>) -> Self {
        self.git_program = program.into();
        self
    }

    /// Overrides one inherited environment variable for each Git child.
    ///
    /// Fixed fixture isolation variables take precedence over these values.
    /// Keys beginning with `GIT_` (case-insensitively) are ignored entirely.
    /// This supports parallel-safe tests of hostile host configuration without
    /// mutating the test process environment.
    pub fn command_environment(
        mut self,
        key: impl AsRef<std::ffi::OsStr>,
        value: impl AsRef<std::ffi::OsStr>,
    ) -> Self {
        self.command_environment
            .push((key.as_ref().to_os_string(), value.as_ref().to_os_string()));
        self
    }

    /// Builds the temporary repository and all fixed commits.
    pub fn build(self) -> Result<RepositoryFixture, FixtureBuildError> {
        let fixture_parent = workspace_root().join("target/assay-fixtures");
        fs::create_dir_all(&fixture_parent)
            .map_err(|error| FixtureBuildError::io(BuildStage::CreateTemporaryDirectory, error))?;
        let temporary_directory = tempfile::Builder::new()
            .prefix("assay-fixture-")
            .tempdir_in(fixture_parent)
            .map_err(|error| FixtureBuildError::io(BuildStage::CreateTemporaryDirectory, error))?;
        let empty_global_config = temporary_directory.path().join("empty-git-config");
        fs::write(&empty_global_config, [])
            .map_err(|error| FixtureBuildError::io(BuildStage::CreateGitConfiguration, error))?;
        let empty_attributes = temporary_directory.path().join("empty-git-attributes");
        fs::write(&empty_attributes, [])
            .map_err(|error| FixtureBuildError::io(BuildStage::CreateGitConfiguration, error))?;
        let empty_excludes = temporary_directory.path().join("empty-git-excludes");
        fs::write(&empty_excludes, [])
            .map_err(|error| FixtureBuildError::io(BuildStage::CreateGitConfiguration, error))?;
        let empty_template = temporary_directory.path().join("empty-git-template");
        fs::create_dir(&empty_template)
            .map_err(|error| FixtureBuildError::io(BuildStage::CreateGitConfiguration, error))?;
        let repository_path = temporary_directory
            .path()
            .join(self.scenario.repository_name());
        fs::create_dir(&repository_path)
            .map_err(|error| FixtureBuildError::io(BuildStage::CreateRepositoryDirectory, error))?;

        let git = GitRunner::new(
            self.git_program,
            empty_global_config,
            self.command_environment,
        );
        let mut init = git.command(temporary_directory.path());
        init.args([
            "init",
            "--quiet",
            "--initial-branch=main",
            "--object-format=sha1",
        ])
        .arg("--template")
        .arg(empty_template)
        .arg(&repository_path);
        git.run(init, BuildStage::InitializeGitRepository)?;

        configure_repository(&git, &repository_path, &empty_attributes, &empty_excludes)?;

        let mut tracked_paths = BTreeSet::new();
        let mut commit_ids = Vec::new();
        for (index, commit) in self.scenario.commits().iter().enumerate() {
            apply_commit_files(&repository_path, commit, &mut tracked_paths)?;
            stage_files(&git, &repository_path, &tracked_paths)?;
            create_commit(&git, &repository_path, commit, index)?;
            commit_ids.push(resolve_head(&git, &repository_path)?);
        }

        Ok(RepositoryFixture {
            _temporary_directory: temporary_directory,
            repository_path,
            scenario: self.scenario,
            commit_ids,
        })
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .expect("assay-test-fixtures must remain under tests/support")
        .to_path_buf()
}

impl fmt::Debug for RepositoryFixtureBuilder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RepositoryFixtureBuilder")
            .field("scenario", &self.scenario)
            .field("git_program", &"<git-program>")
            .field("command_environment", &"<redacted-environment>")
            .finish()
    }
}

/// An owned temporary Git repository that is removed when dropped.
pub struct RepositoryFixture {
    _temporary_directory: TempDir,
    repository_path: PathBuf,
    scenario: RepositoryScenario,
    commit_ids: Vec<String>,
}

impl RepositoryFixture {
    /// Builds one scenario with the default `git` executable.
    pub fn build(scenario: RepositoryScenario) -> Result<Self, FixtureBuildError> {
        RepositoryFixtureBuilder::new(scenario).build()
    }

    /// Returns the repository root for use by integration tests.
    pub fn path(&self) -> &Path {
        &self.repository_path
    }

    /// Returns the scenario used to build this repository.
    pub fn scenario(&self) -> RepositoryScenario {
        self.scenario
    }

    /// Returns commit IDs in chronological order.
    pub fn commit_ids(&self) -> &[String] {
        &self.commit_ids
    }
}

impl fmt::Debug for RepositoryFixture {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RepositoryFixture")
            .field("repository", &"<temporary-repository>")
            .field("scenario", &self.scenario)
            .field("commit_ids", &self.commit_ids)
            .finish()
    }
}
