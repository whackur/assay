//! Deterministic temporary Git repositories for Assay integration tests.
//!
//! This crate is test support only. It creates synthetic repository histories
//! without installing, importing, building, testing, or executing their files.

#![forbid(unsafe_code)]

use std::{
    collections::BTreeSet,
    env,
    error::Error,
    ffi::OsStr,
    fmt, fs, io,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use tempfile::TempDir;

const AUTHOR_NAME: &str = "Assay Fixture Author";
const AUTHOR_EMAIL: &str = "fixture-author@example.invalid";
const COMMITTER_NAME: &str = "Assay Fixture Committer";
const COMMITTER_EMAIL: &str = "fixture-committer@example.invalid";
const FIRST_TIMESTAMP: &str = "2001-02-03T04:05:06+09:00";
const SECOND_TIMESTAMP: &str = "2001-02-04T05:06:07+09:00";

/// The nine synthetic histories required by the Assay foundation milestone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepositoryScenario {
    /// TypeScript production, test, README, license, and CI files.
    TypeScriptProject,
    /// Python production, test, package metadata, and documentation files.
    PythonProject,
    /// A second commit that changes only a dependency manifest and lockfile.
    DependencyOnlyChange,
    /// Generated and vendored paths declared through `.gitattributes`.
    GeneratedAndVendoredOverrides,
    /// A second commit that changes only ASCII formatting.
    FormattingOnlyChange,
    /// An unchanged file renamed and moved by a second commit.
    RenameAndMove,
    /// Supported TypeScript and Python mixed with unsupported Rust and C.
    SupportedAndUnsupportedLanguages,
    /// A repository that intentionally has no README or license.
    MissingReadmeAndLicense,
    /// Spaces and Unicode in both the repository and tracked file paths.
    SpaceAndUnicodePaths,
}

impl RepositoryScenario {
    /// All required scenarios in their stable declaration order.
    pub const ALL: [Self; 9] = [
        Self::TypeScriptProject,
        Self::PythonProject,
        Self::DependencyOnlyChange,
        Self::GeneratedAndVendoredOverrides,
        Self::FormattingOnlyChange,
        Self::RenameAndMove,
        Self::SupportedAndUnsupportedLanguages,
        Self::MissingReadmeAndLicense,
        Self::SpaceAndUnicodePaths,
    ];

    fn repository_name(self) -> &'static str {
        match self {
            Self::SpaceAndUnicodePaths => "fixture repository café",
            _ => "fixture-repository",
        }
    }

    fn commits(self) -> Vec<CommitSpec> {
        match self {
            Self::TypeScriptProject => vec![CommitSpec::new(
                "Add TypeScript project evidence",
                &[
                    FileSpec::new(
                        ".github/workflows/ci.yml",
                        b"name: CI\non: [push]\njobs:\n  test:\n    runs-on: ubuntu-latest\n",
                    ),
                    FileSpec::new(
                        "LICENSE",
                        b"MIT License\n\nCopyright (c) 2001 Assay Fixture\n",
                    ),
                    FileSpec::new(
                        "README.md",
                        b"# TypeScript Fixture\n\nSynthetic repository evidence.\n",
                    ),
                    FileSpec::new(
                        "src/add.ts",
                        b"export function add(left: number, right: number): number {\n  return left + right;\n}\n",
                    ),
                    FileSpec::new(
                        "tests/add.test.ts",
                        b"import { add } from \"../src/add\";\n\nvoid add(1, 2);\n",
                    ),
                ],
                &[],
            )],
            Self::PythonProject => vec![CommitSpec::new(
                "Add Python project evidence",
                &[
                    FileSpec::new(
                        "docs/usage.md",
                        b"# Usage\n\nStatic documentation fixture.\n",
                    ),
                    FileSpec::new(
                        "pyproject.toml",
                        b"[project]\nname = \"assay-fixture\"\nversion = \"0.1.0\"\n",
                    ),
                    FileSpec::new(
                        "src/assay_fixture/__init__.py",
                        b"def add(left: int, right: int) -> int:\n    return left + right\n",
                    ),
                    FileSpec::new(
                        "tests/test_add.py",
                        b"from assay_fixture import add\n\n\ndef test_add() -> None:\n    assert add(1, 2) == 3\n",
                    ),
                ],
                &[],
            )],
            Self::DependencyOnlyChange => vec![
                CommitSpec::new(
                    "Add dependency fixture",
                    &[
                        FileSpec::new(
                            "package-lock.json",
                            b"{\n  \"lockfileVersion\": 3,\n  \"packages\": {\"\": {\"dependencies\": {\"left-pad\": \"1.2.0\"}}}\n}\n",
                        ),
                        FileSpec::new(
                            "package.json",
                            b"{\n  \"name\": \"dependency-fixture\",\n  \"dependencies\": {\"left-pad\": \"1.2.0\"}\n}\n",
                        ),
                        FileSpec::new("src/index.ts", b"export const value = 1;\n"),
                    ],
                    &[],
                ),
                CommitSpec::new(
                    "Update dependencies only",
                    &[
                        FileSpec::new(
                            "package-lock.json",
                            b"{\n  \"lockfileVersion\": 3,\n  \"packages\": {\"\": {\"dependencies\": {\"left-pad\": \"1.3.0\"}}}\n}\n",
                        ),
                        FileSpec::new(
                            "package.json",
                            b"{\n  \"name\": \"dependency-fixture\",\n  \"dependencies\": {\"left-pad\": \"1.3.0\"}\n}\n",
                        ),
                    ],
                    &[],
                ),
            ],
            Self::GeneratedAndVendoredOverrides => vec![CommitSpec::new(
                "Add generated and vendored overrides",
                &[
                    FileSpec::new(
                        ".gitattributes",
                        b"generated/** linguist-generated=true\nvendor/** linguist-vendored=true\n",
                    ),
                    FileSpec::new(
                        "generated/client.ts",
                        b"export const generatedClient = true;\n",
                    ),
                    FileSpec::new(
                        "src/main.ts",
                        b"export const application = true;\n",
                    ),
                    FileSpec::new("vendor/library.py", b"VENDORED_VALUE = True\n"),
                ],
                &[],
            )],
            Self::FormattingOnlyChange => vec![
                CommitSpec::new(
                    "Add compact source",
                    &[FileSpec::new(
                        "src/format.ts",
                        b"export function format(value:string):string{return value.trim();}\n",
                    )],
                    &[],
                ),
                CommitSpec::new(
                    "Format source only",
                    &[FileSpec::new(
                        "src/format.ts",
                        b"export function format(value: string): string {\n  return value.trim();\n}\n",
                    )],
                    &[],
                ),
            ],
            Self::RenameAndMove => vec![
                CommitSpec::new(
                    "Add legacy module",
                    &[FileSpec::new(
                        "src/legacy.ts",
                        b"export const stableValue = 42;\n",
                    )],
                    &[],
                ),
                CommitSpec::new(
                    "Rename and move module",
                    &[FileSpec::new(
                        "src/core/renamed.ts",
                        b"export const stableValue = 42;\n",
                    )],
                    &["src/legacy.ts"],
                ),
            ],
            Self::SupportedAndUnsupportedLanguages => vec![CommitSpec::new(
                "Add mixed-language sources",
                &[
                    FileSpec::new("native/tool.c", b"int answer(void) { return 42; }\n"),
                    FileSpec::new("src/main.rs", b"pub fn answer() -> u8 { 42 }\n"),
                    FileSpec::new(
                        "src/main.ts",
                        b"export const answer: number = 42;\n",
                    ),
                    FileSpec::new("src/tool.py", b"def answer() -> int:\n    return 42\n"),
                ],
                &[],
            )],
            Self::MissingReadmeAndLicense => vec![CommitSpec::new(
                "Add project without community files",
                &[
                    FileSpec::new(
                        "package.json",
                        b"{\n  \"name\": \"missing-community-files\"\n}\n",
                    ),
                    FileSpec::new(
                        "src/main.ts",
                        b"export const documented = false;\n",
                    ),
                ],
                &[],
            )],
            Self::SpaceAndUnicodePaths => vec![CommitSpec::new(
                "Add space and Unicode paths",
                &[
                    FileSpec::new(
                        "docs/résumé.md",
                        b"# Resume\n\nSynthetic Unicode fixture.\n",
                    ),
                    FileSpec::new(
                        "src/hello world.ts",
                        b"export const greeting = \"hello\";\n",
                    ),
                ],
                &[],
            )],
        }
    }
}

/// Builder for one isolated deterministic repository fixture.
pub struct RepositoryFixtureBuilder {
    scenario: RepositoryScenario,
    git_program: PathBuf,
}

impl RepositoryFixtureBuilder {
    /// Creates a builder that invokes `git` directly without a shell.
    pub fn new(scenario: RepositoryScenario) -> Self {
        Self {
            scenario,
            git_program: PathBuf::from("git"),
        }
    }

    /// Selects a Git executable path, primarily for hermetic failure tests.
    pub fn git_program(mut self, program: impl Into<PathBuf>) -> Self {
        self.git_program = program.into();
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
        let empty_template = temporary_directory.path().join("empty-git-template");
        fs::create_dir(&empty_template)
            .map_err(|error| FixtureBuildError::io(BuildStage::CreateGitConfiguration, error))?;
        let repository_path = temporary_directory
            .path()
            .join(self.scenario.repository_name());
        fs::create_dir(&repository_path)
            .map_err(|error| FixtureBuildError::io(BuildStage::CreateRepositoryDirectory, error))?;

        let git = GitRunner::new(self.git_program, empty_global_config);
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

        configure_repository(&git, &repository_path)?;

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

/// Redacted failure information for fixture creation.
pub struct FixtureBuildError {
    stage: BuildStage,
    reason: FailureReason,
}

impl FixtureBuildError {
    fn io(stage: BuildStage, error: io::Error) -> Self {
        Self {
            stage,
            reason: FailureReason::Io(error.kind()),
        }
    }

    fn git(stage: BuildStage, output: &Output) -> Self {
        Self {
            stage,
            reason: FailureReason::GitExit(output.status.code()),
        }
    }

    fn invalid_utf8(stage: BuildStage) -> Self {
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

impl Error for FixtureBuildError {}

#[derive(Clone, Copy, Debug)]
enum BuildStage {
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
enum FailureReason {
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

struct GitRunner {
    program: PathBuf,
    global_config: PathBuf,
}

impl GitRunner {
    fn new(program: PathBuf, global_config: PathBuf) -> Self {
        Self {
            program,
            global_config,
        }
    }

    fn command(&self, current_directory: &Path) -> Command {
        let mut command = Command::new(&self.program);
        remove_git_environment(&mut command);
        command
            .current_dir(current_directory)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", &self.global_config)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("LC_ALL", "C")
            .env("TZ", "UTC");
        command
    }

    fn run(&self, mut command: Command, stage: BuildStage) -> Result<Output, FixtureBuildError> {
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

fn is_git_environment_key(key: &OsStr) -> bool {
    key.to_string_lossy().starts_with("GIT_")
}

fn configure_repository(git: &GitRunner, repository: &Path) -> Result<(), FixtureBuildError> {
    for (key, value) in [
        ("commit.cleanup", "verbatim"),
        ("commit.gpgSign", "false"),
        ("core.autocrlf", "false"),
        ("core.fileMode", "false"),
        ("core.ignoreCase", "false"),
        ("core.precomposeUnicode", "false"),
        ("core.quotePath", "false"),
        ("i18n.commitEncoding", "UTF-8"),
        ("init.defaultBranch", "main"),
        ("user.email", COMMITTER_EMAIL),
        ("user.name", COMMITTER_NAME),
        ("user.useConfigOnly", "true"),
    ] {
        let mut command = git.command(repository);
        command.args(["config", "--local", key, value]);
        git.run(command, BuildStage::ConfigureGitRepository)?;
    }
    Ok(())
}

fn apply_commit_files(
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

fn stage_files(
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

fn create_commit(
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

fn resolve_head(git: &GitRunner, repository: &Path) -> Result<String, FixtureBuildError> {
    let mut command = git.command(repository);
    command.args(["rev-parse", "--verify", "HEAD"]);
    let output = git.run(command, BuildStage::ResolveCommit)?;
    let commit_id = String::from_utf8(output.stdout)
        .map_err(|_| FixtureBuildError::invalid_utf8(BuildStage::ResolveCommit))?;
    Ok(commit_id.trim_end().to_owned())
}

struct CommitSpec {
    message: &'static str,
    files: Vec<FileSpec>,
    removals: Vec<&'static str>,
}

impl CommitSpec {
    fn new(message: &'static str, files: &[FileSpec], removals: &[&'static str]) -> Self {
        Self {
            message,
            files: files.to_vec(),
            removals: removals.to_vec(),
        }
    }
}

#[derive(Clone, Copy)]
struct FileSpec {
    path: &'static str,
    contents: &'static [u8],
}

impl FileSpec {
    const fn new(path: &'static str, contents: &'static [u8]) -> Self {
        Self { path, contents }
    }
}
