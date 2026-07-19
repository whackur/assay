//! Git command runner and repository configuration helpers.

use std::{
    collections::BTreeSet,
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use crate::error::{BuildStage, FixtureBuildError};
use crate::spec::CommitSpec;

const AUTHOR_NAME: &str = "Assay Fixture Author";
const AUTHOR_EMAIL: &str = "fixture-author@example.invalid";
const COMMITTER_NAME: &str = "Assay Fixture Committer";
const COMMITTER_EMAIL: &str = "fixture-committer@example.invalid";
const FIRST_TIMESTAMP: &str = "2001-02-03T04:05:06+09:00";
const SECOND_TIMESTAMP: &str = "2001-02-04T05:06:07+09:00";

pub(crate) struct GitRunner {
    program: PathBuf,
    global_config: PathBuf,
    command_environment: Vec<(std::ffi::OsString, std::ffi::OsString)>,
}

impl GitRunner {
    pub(crate) fn new(
        program: PathBuf,
        global_config: PathBuf,
        command_environment: Vec<(std::ffi::OsString, std::ffi::OsString)>,
    ) -> Self {
        Self {
            program,
            global_config,
            command_environment,
        }
    }

    pub(crate) fn command(&self, current_directory: &Path) -> Command {
        let mut command = Command::new(&self.program);
        remove_git_environment(&mut command);
        command
            .current_dir(current_directory)
            .envs(
                self.command_environment
                    .iter()
                    .filter(|(key, _)| !is_git_environment_key(key))
                    .cloned(),
            )
            .env("GIT_ATTR_NOSYSTEM", "1")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", &self.global_config)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("LC_ALL", "C")
            .env("TZ", "UTC");
        command
    }

    pub(crate) fn run(
        &self,
        mut command: Command,
        stage: BuildStage,
    ) -> Result<Output, FixtureBuildError> {
        let output = command
            .output()
            .map_err(|error| FixtureBuildError::io(stage, error))?;
        if output.status.success() {
            Ok(output)
        } else {
            Err(FixtureBuildError::git(stage, &output))
        }
    }
}

fn remove_git_environment(command: &mut Command) {
    for (key, _) in env::vars_os() {
        if is_git_environment_key(&key) {
            command.env_remove(key);
        }
    }
    for key in ["EMAIL", "LANG", "LC_ALL", "TZ"] {
        command.env_remove(key);
    }
}

fn is_git_environment_key(key: &std::ffi::OsStr) -> bool {
    key.to_string_lossy()
        .get(..4)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("GIT_"))
}

pub(crate) fn configure_repository(
    git: &GitRunner,
    repository: &Path,
    attributes_file: &Path,
    excludes_file: &Path,
) -> Result<(), FixtureBuildError> {
    for (key, value) in [
        ("commit.cleanup", "verbatim"),
        ("commit.gpgSign", "false"),
        ("core.autocrlf", "false"),
        ("core.fileMode", "false"),
        ("core.ignoreCase", "false"),
        ("core.precomposeUnicode", "true"),
        ("core.quotePath", "false"),
        ("i18n.commitEncoding", "UTF-8"),
        ("init.defaultBranch", "main"),
        ("user.email", COMMITTER_EMAIL),
        ("user.name", COMMITTER_NAME),
        ("user.useConfigOnly", "true"),
    ] {
        set_local_config(git, repository, key, value)?;
    }
    set_local_config(git, repository, "core.attributesFile", attributes_file)?;
    set_local_config(git, repository, "core.excludesFile", excludes_file)?;
    Ok(())
}

fn set_local_config(
    git: &GitRunner,
    repository: &Path,
    key: &str,
    value: impl AsRef<std::ffi::OsStr>,
) -> Result<(), FixtureBuildError> {
    let mut command = git.command(repository);
    command.args(["config", "--local", key]).arg(value.as_ref());
    git.run(command, BuildStage::ConfigureGitRepository)?;
    Ok(())
}

pub(crate) fn apply_commit_files(
    repository: &Path,
    commit: &CommitSpec,
    tracked_paths: &mut BTreeSet<&'static str>,
) -> Result<(), FixtureBuildError> {
    for relative_path in &commit.removals {
        fs::remove_file(repository.join(*relative_path))
            .map_err(|error| FixtureBuildError::io(BuildStage::RemoveFixtureFile, error))?;
        tracked_paths.remove(*relative_path);
    }

    for file in &commit.files {
        let destination = repository.join(file.path);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| FixtureBuildError::io(BuildStage::WriteFixtureFile, error))?;
        }
        fs::write(&destination, file.contents)
            .map_err(|error| FixtureBuildError::io(BuildStage::WriteFixtureFile, error))?;
        set_regular_file_permissions(&destination)?;
        tracked_paths.insert(file.path);
    }
    Ok(())
}

#[cfg(unix)]
fn set_regular_file_permissions(path: &Path) -> Result<(), FixtureBuildError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o644))
        .map_err(|error| FixtureBuildError::io(BuildStage::WriteFixtureFile, error))
}

#[cfg(not(unix))]
fn set_regular_file_permissions(_path: &Path) -> Result<(), FixtureBuildError> {
    Ok(())
}

pub(crate) fn stage_files(
    git: &GitRunner,
    repository: &Path,
    tracked_paths: &BTreeSet<&str>,
) -> Result<(), FixtureBuildError> {
    let mut add = git.command(repository);
    add.args(["add", "--all", "--", "."]);
    git.run(add, BuildStage::StageFixtureFiles)?;

    for relative_path in tracked_paths {
        let mut normalize_mode = git.command(repository);
        normalize_mode
            .args(["update-index", "--chmod=-x", "--"])
            .arg(relative_path);
        git.run(normalize_mode, BuildStage::NormalizeFileMode)?;
    }
    Ok(())
}

pub(crate) fn create_commit(
    git: &GitRunner,
    repository: &Path,
    commit: &CommitSpec,
    index: usize,
) -> Result<(), FixtureBuildError> {
    let timestamp = match index {
        0 => FIRST_TIMESTAMP,
        1 => SECOND_TIMESTAMP,
        _ => SECOND_TIMESTAMP,
    };
    let mut command = git.command(repository);
    command
        .env("GIT_AUTHOR_NAME", AUTHOR_NAME)
        .env("GIT_AUTHOR_EMAIL", AUTHOR_EMAIL)
        .env("GIT_AUTHOR_DATE", timestamp)
        .env("GIT_COMMITTER_NAME", COMMITTER_NAME)
        .env("GIT_COMMITTER_EMAIL", COMMITTER_EMAIL)
        .env("GIT_COMMITTER_DATE", timestamp)
        .args([
            "commit",
            "--quiet",
            "--no-gpg-sign",
            "--cleanup=verbatim",
            "-m",
            commit.message,
        ]);
    git.run(command, BuildStage::CreateCommit)?;
    Ok(())
}

pub(crate) fn resolve_head(
    git: &GitRunner,
    repository: &Path,
) -> Result<String, FixtureBuildError> {
    let mut command = git.command(repository);
    command.args(["rev-parse", "--verify", "HEAD"]);
    let output = git.run(command, BuildStage::ResolveCommit)?;
    let commit_id = String::from_utf8(output.stdout)
        .map_err(|_| FixtureBuildError::invalid_utf8(BuildStage::ResolveCommit))?;
    Ok(commit_id.trim_end().to_owned())
}
