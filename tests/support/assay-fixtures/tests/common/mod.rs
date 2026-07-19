//! Shared helpers for the repository scenario integration tests.
#![allow(dead_code)]

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use assay_test_fixtures::RepositoryScenario;

pub(crate) struct ScenarioExpectation {
    pub scenario: RepositoryScenario,
    pub commits: Vec<CommitExpectation>,
}

pub(crate) struct CommitExpectation {
    pub message: &'static str,
    pub files: BTreeMap<&'static str, &'static [u8]>,
}

pub(crate) fn git_output(repository: &Path, arguments: &[&str]) -> Output {
    let mut command = Command::new("git");
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("GIT_") {
            command.env_remove(key);
        }
    }
    command
        .current_dir(repository)
        .env("GIT_ATTR_NOSYSTEM", "1")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .env("TZ", "UTC")
        .args(arguments);
    let output = command
        .output()
        .expect("Git must be available for fixtures");
    assert!(output.status.success(), "fixture Git query failed");
    output
}

pub(crate) fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .expect("the fixture crate must remain under tests/support")
        .to_path_buf()
}

pub(crate) fn git_text(repository: &Path, arguments: &[&str]) -> String {
    let output = git_output(repository, arguments);
    String::from_utf8(output.stdout)
        .expect("fixture Git output must be UTF-8")
        .trim_end()
        .to_owned()
}

pub(crate) fn git_lines(repository: &Path, arguments: &[&str]) -> Vec<String> {
    git_text(repository, arguments)
        .lines()
        .map(str::to_owned)
        .collect()
}

pub(crate) fn expected_timestamp(commit_index: usize) -> &'static str {
    match commit_index {
        0 => "2001-02-03T04:05:06+09:00",
        1 => "2001-02-04T05:06:07+09:00",
        _ => panic!("the initial scenarios use at most two commits"),
    }
}

pub(crate) fn without_ascii_whitespace(bytes: &[u8]) -> Vec<u8> {
    bytes
        .iter()
        .copied()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect()
}

pub(crate) fn expectations() -> Vec<ScenarioExpectation> {
    vec![
        expectation(
            RepositoryScenario::TypeScriptProject,
            &["Add TypeScript project evidence"],
            &[
                (".github/workflows/ci.yml", b"name: CI\non: [push]\njobs:\n  test:\n    runs-on: ubuntu-latest\n"),
                ("LICENSE", b"MIT License\n\nCopyright (c) 2001 Assay Fixture\n"),
                ("README.md", b"# TypeScript Fixture\n\nSynthetic repository evidence.\n"),
                ("src/add.ts", b"export function add(left: number, right: number): number {\n  return left + right;\n}\n"),
                ("tests/add.test.ts", b"import { add } from \"../src/add\";\n\nvoid add(1, 2);\n"),
            ],
        ),
        expectation(
            RepositoryScenario::PythonProject,
            &["Add Python project evidence"],
            &[
                ("docs/usage.md", b"# Usage\n\nStatic documentation fixture.\n"),
                ("pyproject.toml", b"[project]\nname = \"assay-fixture\"\nversion = \"0.1.0\"\n"),
                ("src/assay_fixture/__init__.py", b"def add(left: int, right: int) -> int:\n    return left + right\n"),
                ("tests/test_add.py", b"from assay_fixture import add\n\n\ndef test_add() -> None:\n    assert add(1, 2) == 3\n"),
            ],
        ),
        history_expectation(
            RepositoryScenario::DependencyOnlyChange,
            vec![
                commit_expectation(
                    "Add dependency fixture",
                    &[
                        ("package-lock.json", b"{\n  \"lockfileVersion\": 3,\n  \"packages\": {\"\": {\"dependencies\": {\"left-pad\": \"1.2.0\"}}}\n}\n"),
                        ("package.json", b"{\n  \"name\": \"dependency-fixture\",\n  \"dependencies\": {\"left-pad\": \"1.2.0\"}\n}\n"),
                        ("src/index.ts", b"export const value = 1;\n"),
                    ],
                ),
                commit_expectation(
                    "Update dependencies only",
                    &[
                        ("package-lock.json", b"{\n  \"lockfileVersion\": 3,\n  \"packages\": {\"\": {\"dependencies\": {\"left-pad\": \"1.3.0\"}}}\n}\n"),
                        ("package.json", b"{\n  \"name\": \"dependency-fixture\",\n  \"dependencies\": {\"left-pad\": \"1.3.0\"}\n}\n"),
                        ("src/index.ts", b"export const value = 1;\n"),
                    ],
                ),
            ],
        ),
        expectation(
            RepositoryScenario::GeneratedAndVendoredOverrides,
            &["Add generated and vendored overrides"],
            &[
                (".gitattributes", b"generated/** linguist-generated=true\nvendor/** linguist-vendored=true\n"),
                ("generated/client.ts", b"export const generatedClient = true;\n"),
                ("src/main.ts", b"export const application = true;\n"),
                ("vendor/library.py", b"VENDORED_VALUE = True\n"),
            ],
        ),
        history_expectation(
            RepositoryScenario::FormattingOnlyChange,
            vec![
                commit_expectation(
                    "Add compact source",
                    &[("src/format.ts", b"export function format(value:string):string{return value.trim();}\n")],
                ),
                commit_expectation(
                    "Format source only",
                    &[("src/format.ts", b"export function format(value: string): string {\n  return value.trim();\n}\n")],
                ),
            ],
        ),
        history_expectation(
            RepositoryScenario::RenameAndMove,
            vec![
                commit_expectation(
                    "Add legacy module",
                    &[("src/legacy.ts", b"export const stableValue = 42;\n")],
                ),
                commit_expectation(
                    "Rename and move module",
                    &[("src/core/renamed.ts", b"export const stableValue = 42;\n")],
                ),
            ],
        ),
        expectation(
            RepositoryScenario::SupportedAndUnsupportedLanguages,
            &["Add mixed-language sources"],
            &[
                ("native/tool.c", b"int answer(void) { return 42; }\n"),
                ("src/main.rs", b"pub fn answer() -> u8 { 42 }\n"),
                ("src/main.ts", b"export const answer: number = 42;\n"),
                ("src/tool.py", b"def answer() -> int:\n    return 42\n"),
            ],
        ),
        expectation(
            RepositoryScenario::MissingReadmeAndLicense,
            &["Add project without community files"],
            &[
                ("package.json", b"{\n  \"name\": \"missing-community-files\"\n}\n"),
                ("src/main.ts", b"export const documented = false;\n"),
            ],
        ),
        expectation(
            RepositoryScenario::SpaceAndUnicodePaths,
            &["Add space and Unicode paths"],
            &[
                ("docs/résumé.md", b"# Resume\n\nSynthetic Unicode fixture.\n"),
                ("src/hello world.ts", b"export const greeting = \"hello\";\n"),
            ],
        ),
    ]
}

fn expectation(
    scenario: RepositoryScenario,
    messages: &[&'static str],
    files: &[(&'static str, &'static [u8])],
) -> ScenarioExpectation {
    assert_eq!(
        messages.len(),
        1,
        "single-commit golden requires one message"
    );
    ScenarioExpectation {
        scenario,
        commits: vec![commit_expectation(messages[0], files)],
    }
}

fn history_expectation(
    scenario: RepositoryScenario,
    commits: Vec<CommitExpectation>,
) -> ScenarioExpectation {
    ScenarioExpectation { scenario, commits }
}

fn commit_expectation(
    message: &'static str,
    files: &[(&'static str, &'static [u8])],
) -> CommitExpectation {
    CommitExpectation {
        message,
        files: files.iter().copied().collect(),
    }
}
