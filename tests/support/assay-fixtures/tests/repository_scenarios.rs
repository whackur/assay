use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use assay_test_fixtures::{RepositoryFixture, RepositoryFixtureBuilder, RepositoryScenario};

const AUTHOR_NAME: &str = "Assay Fixture Author";
const AUTHOR_EMAIL: &str = "fixture-author@example.invalid";
const COMMITTER_NAME: &str = "Assay Fixture Committer";
const COMMITTER_EMAIL: &str = "fixture-committer@example.invalid";

#[test]
fn every_required_repository_scenario_has_exact_files_and_history() {
    for expected in expectations() {
        let fixture = RepositoryFixture::build(expected.scenario)
            .expect("the deterministic repository fixture must build");

        assert_eq!(fixture.scenario(), expected.scenario);
        assert_eq!(fixture.commit_ids().len(), expected.commits.len());
        assert_eq!(
            git_text(fixture.path(), &["branch", "--show-current"]),
            "main"
        );
        assert_eq!(
            git_text(fixture.path(), &["rev-parse", "--show-object-format"]),
            "sha1"
        );

        let actual_messages = git_lines(fixture.path(), &["log", "--reverse", "--format=%s"]);
        let expected_messages = expected
            .commits
            .iter()
            .map(|commit| commit.message.to_owned())
            .collect::<Vec<_>>();
        assert_eq!(actual_messages, expected_messages);

        for (index, expected_commit) in expected.commits.iter().enumerate() {
            let commit_id = &fixture.commit_ids()[index];
            let actual_files =
                git_lines(fixture.path(), &["ls-tree", "-r", "--name-only", commit_id]);
            let expected_paths = expected_commit.files.keys().copied().collect::<Vec<_>>();
            assert_eq!(actual_files, expected_paths);

            for (relative_path, expected_bytes) in &expected_commit.files {
                let object = format!("{commit_id}:{relative_path}");
                let committed_bytes =
                    git_output(fixture.path(), &["cat-file", "blob", object.as_str()]).stdout;
                assert!(
                    committed_bytes.as_slice() == *expected_bytes,
                    "committed fixture blob bytes changed for {relative_path}"
                );

                if index + 1 == expected.commits.len() {
                    let working_tree_bytes = fs::read(fixture.path().join(relative_path))
                        .expect("a final fixture file must be readable");
                    assert!(
                        working_tree_bytes.as_slice() == *expected_bytes,
                        "working-tree fixture bytes changed for {relative_path}"
                    );
                }
            }

            let actual_modes = git_lines(
                fixture.path(),
                &["ls-tree", "-r", "--format=%(objectmode) %(path)", commit_id],
            );
            let expected_modes = expected_commit
                .files
                .keys()
                .map(|path| format!("100644 {path}"))
                .collect::<Vec<_>>();
            assert_eq!(actual_modes, expected_modes);
        }
    }
}

#[test]
fn author_committer_dates_timezones_and_commit_order_are_fixed() {
    for scenario in RepositoryScenario::ALL {
        let fixture = RepositoryFixture::build(scenario)
            .expect("the deterministic repository fixture must build");
        let records = git_lines(
            fixture.path(),
            &[
                "log",
                "--reverse",
                "--format=%an%x00%ae%x00%aI%x00%cn%x00%ce%x00%cI%x00%H",
            ],
        );

        assert_eq!(records.len(), fixture.commit_ids().len());
        for (index, record) in records.iter().enumerate() {
            let fields = record.split('\0').collect::<Vec<_>>();
            assert_eq!(fields.len(), 7);
            assert_eq!(fields[0], AUTHOR_NAME);
            assert_eq!(fields[1], AUTHOR_EMAIL);
            assert_eq!(fields[2], expected_timestamp(index));
            assert_eq!(fields[3], COMMITTER_NAME);
            assert_eq!(fields[4], COMMITTER_EMAIL);
            assert_eq!(fields[5], expected_timestamp(index));
            assert_eq!(fields[6], fixture.commit_ids()[index]);
        }
    }
}

#[test]
fn independent_builds_have_identical_sha1_commit_and_tree_ids() {
    for scenario in RepositoryScenario::ALL {
        let first = RepositoryFixture::build(scenario)
            .expect("the first deterministic repository fixture must build");
        let second = RepositoryFixture::build(scenario)
            .expect("the second deterministic repository fixture must build");

        assert_ne!(first.path(), second.path());
        assert_eq!(first.commit_ids(), second.commit_ids());
        assert_eq!(
            git_text(first.path(), &["rev-parse", "HEAD^{tree}"]),
            git_text(second.path(), &["rev-parse", "HEAD^{tree}"])
        );
    }
}

#[test]
fn repository_config_is_isolated_and_fixed() {
    let fixture = RepositoryFixture::build(RepositoryScenario::TypeScriptProject)
        .expect("the deterministic repository fixture must build");

    for (key, expected) in [
        ("commit.gpgsign", "false"),
        ("core.autocrlf", "false"),
        ("core.filemode", "false"),
        ("core.ignorecase", "false"),
        ("core.precomposeunicode", "true"),
        ("core.quotepath", "false"),
        ("i18n.commitencoding", "UTF-8"),
        ("init.defaultbranch", "main"),
        ("user.email", COMMITTER_EMAIL),
        ("user.name", COMMITTER_NAME),
        ("user.useconfigonly", "true"),
    ] {
        assert_eq!(
            git_text(fixture.path(), &["config", "--local", "--get", key]),
            expected,
            "unexpected local Git configuration for {key}"
        );
    }

    for key in ["core.attributesfile", "core.excludesfile"] {
        let configured_path = PathBuf::from(git_text(
            fixture.path(),
            &["config", "--local", "--get", key],
        ));
        assert!(
            configured_path.starts_with(
                fixture
                    .path()
                    .parent()
                    .expect("fixture repository must have an owned temporary parent")
            )
        );
        assert!(
            fs::read(configured_path)
                .expect("the isolated Git policy file must be readable")
                .is_empty(),
            "the isolated Git policy file must remain empty"
        );
    }
}

#[test]
fn adversarial_external_attributes_and_ignores_cannot_change_the_fixture() {
    let host_parent = workspace_root().join("target/assay-fixture-host-configs");
    fs::create_dir_all(&host_parent).expect("the host configuration parent must be creatable");
    let host = tempfile::Builder::new()
        .prefix("adversarial-host-")
        .tempdir_in(host_parent)
        .expect("the adversarial host configuration must be creatable");
    let home = host.path().join("home");
    let xdg = host.path().join("xdg");
    fs::create_dir_all(xdg.join("git")).expect("the fake XDG Git directory must be creatable");
    fs::create_dir_all(&home).expect("the fake home directory must be creatable");

    let attributes = host.path().join("host-attributes");
    let excludes = host.path().join("host-ignore");
    fs::write(&attributes, b"*.ts working-tree-encoding=UTF-16LE\n")
        .expect("the adversarial attributes file must be writable");
    fs::write(&excludes, b"*\n").expect("the adversarial ignore file must be writable");
    fs::write(
        xdg.join("git/attributes"),
        b"*.ts working-tree-encoding=UTF-16LE\n",
    )
    .expect("the default XDG attributes file must be writable");
    fs::write(xdg.join("git/ignore"), b"*\n")
        .expect("the default XDG ignore file must be writable");
    let config = format!(
        "[core]\n\tattributesFile = {}\n\texcludesFile = {}\n",
        attributes.display(),
        excludes.display()
    );
    let home_config = home.join(".gitconfig");
    fs::write(&home_config, &config).expect("the fake home Git config must be writable");
    fs::write(xdg.join("git/config"), config).expect("the fake XDG Git config must be writable");

    let fixture = RepositoryFixtureBuilder::new(RepositoryScenario::TypeScriptProject)
        .command_environment("HOME", &home)
        .command_environment("XDG_CONFIG_HOME", &xdg)
        .command_environment("GIT_CONFIG_GLOBAL", &home_config)
        .command_environment("GIT_CONFIG_NOSYSTEM", "0")
        .command_environment("GIT_ATTR_NOSYSTEM", "0")
        .build()
        .expect("host attributes and ignores must not affect fixture creation");
    let reference = RepositoryFixture::build(RepositoryScenario::TypeScriptProject)
        .expect("the isolated reference fixture must build");

    assert!(fixture.path().join("src/add.ts").is_file());
    assert_eq!(fixture.commit_ids(), reference.commit_ids());
    assert_eq!(
        git_text(fixture.path(), &["rev-parse", "HEAD^{tree}"]),
        git_text(reference.path(), &["rev-parse", "HEAD^{tree}"])
    );
}

#[test]
fn forbidden_git_environment_cannot_redirect_fixture_boundaries() {
    let host_parent = workspace_root().join("target/assay-fixture-git-boundaries");
    fs::create_dir_all(&host_parent).expect("the boundary test parent must be creatable");
    let host = tempfile::Builder::new()
        .prefix("forbidden-git-environment-")
        .tempdir_in(host_parent)
        .expect("the boundary test directory must be creatable");
    let external_index = host.path().join("external-index");
    let external_objects = host.path().join("external-objects");
    let external_git_dir = host.path().join("external-git-dir");
    let external_work_tree = host.path().join("external-work-tree");
    let external_hooks = host.path().join("external-hooks");
    let external_ignore = host.path().join("external-ignore");
    fs::create_dir(&external_hooks).expect("the external hooks directory must be creatable");
    fs::write(&external_ignore, b"*\n").expect("the external ignore file must be writable");

    let fixture = RepositoryFixtureBuilder::new(RepositoryScenario::TypeScriptProject)
        .command_environment("GIT_INDEX_FILE", &external_index)
        .command_environment("GIT_OBJECT_DIRECTORY", &external_objects)
        .command_environment("GIT_ALTERNATE_OBJECT_DIRECTORIES", &external_objects)
        .command_environment("GIT_DIR", &external_git_dir)
        .command_environment("GIT_WORK_TREE", &external_work_tree)
        .command_environment("GIT_CONFIG_COUNT", "2")
        .command_environment("GIT_CONFIG_KEY_0", "core.hooksPath")
        .command_environment("GIT_CONFIG_VALUE_0", &external_hooks)
        .command_environment("GIT_CONFIG_KEY_1", "core.excludesFile")
        .command_environment("GIT_CONFIG_VALUE_1", &external_ignore)
        .build()
        .expect("forbidden Git environment must not redirect fixture boundaries");
    let reference = RepositoryFixture::build(RepositoryScenario::TypeScriptProject)
        .expect("the isolated reference fixture must build");

    assert_eq!(fixture.commit_ids(), reference.commit_ids());
    assert_eq!(
        git_text(fixture.path(), &["rev-parse", "HEAD^{tree}"]),
        git_text(reference.path(), &["rev-parse", "HEAD^{tree}"])
    );
    for external_path in [
        external_index,
        external_objects,
        external_git_dir,
        external_work_tree,
    ] {
        assert!(!external_path.exists());
    }
    assert!(
        fs::read_dir(external_hooks)
            .expect("the external hooks directory must be readable")
            .next()
            .is_none()
    );
}

#[test]
fn fixture_initialization_installs_no_git_hooks() {
    let fixture = RepositoryFixture::build(RepositoryScenario::TypeScriptProject)
        .expect("the deterministic repository fixture must build");
    let hooks = fixture.path().join(".git/hooks");

    assert!(
        !hooks.exists()
            || fs::read_dir(hooks)
                .expect("the fixture hooks directory must be readable")
                .next()
                .is_none()
    );
}

#[test]
fn unicode_scenario_uses_a_space_and_unicode_repository_path() {
    let fixture = RepositoryFixture::build(RepositoryScenario::SpaceAndUnicodePaths)
        .expect("the deterministic repository fixture must build");
    let repository_name = fixture
        .path()
        .file_name()
        .and_then(|name| name.to_str())
        .expect("fixture repository name must be UTF-8");

    assert!(repository_name.contains(' '));
    assert!(repository_name.contains("café"));
    assert!(fixture.path().join("src/hello world.ts").is_file());
    assert!(fixture.path().join("docs/résumé.md").is_file());
}

#[test]
fn fixture_repositories_stay_inside_the_workspace_target_directory() {
    let fixture = RepositoryFixture::build(RepositoryScenario::TypeScriptProject)
        .expect("the deterministic repository fixture must build");
    assert!(fixture.path().starts_with(workspace_root().join("target")));
}

#[test]
fn fixture_debug_output_redacts_paths_and_source_content() {
    let fixture = RepositoryFixture::build(RepositoryScenario::TypeScriptProject)
        .expect("the deterministic repository fixture must build");
    let debug = format!("{fixture:?}");
    let machine_path = fixture.path().to_string_lossy();

    assert!(!debug.contains(machine_path.as_ref()));
    assert!(!debug.contains("export function add"));
    assert!(!debug.contains("example.invalid"));
    assert!(debug.contains("<temporary-repository>"));
}

#[test]
fn fixture_build_errors_redact_program_paths_credentials_and_source_content() {
    let sensitive_program = "/machine/private/token/export function add";
    let builder = RepositoryFixtureBuilder::new(RepositoryScenario::TypeScriptProject)
        .git_program(sensitive_program)
        .command_environment("SENSITIVE_FIXTURE_TOKEN", "credential-value");
    let builder_debug = format!("{builder:?}");
    assert!(!builder_debug.contains(sensitive_program));
    assert!(!builder_debug.contains("credential-value"));

    let error = builder
        .build()
        .expect_err("a missing Git executable must fail safely");
    let diagnostics = format!("{error:?} {error}");
    assert!(!diagnostics.contains(sensitive_program));
    assert!(!diagnostics.contains("example.invalid"));
    assert!(!diagnostics.contains("credential-value"));
    assert!(!diagnostics.contains("export function add"));
    assert!(!diagnostics.contains("diff --git"));
}

#[test]
fn dependency_update_commit_changes_only_manifest_and_lockfile() {
    let fixture = RepositoryFixture::build(RepositoryScenario::DependencyOnlyChange)
        .expect("the deterministic repository fixture must build");
    assert_eq!(
        git_lines(
            fixture.path(),
            &["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"]
        ),
        ["package-lock.json", "package.json"]
    );
}

#[test]
fn gitattributes_declare_generated_and_vendored_overrides() {
    let fixture = RepositoryFixture::build(RepositoryScenario::GeneratedAndVendoredOverrides)
        .expect("the deterministic repository fixture must build");
    assert_eq!(
        git_text(
            fixture.path(),
            &[
                "check-attr",
                "linguist-generated",
                "--",
                "generated/client.ts"
            ]
        ),
        "generated/client.ts: linguist-generated: true"
    );
    assert_eq!(
        git_text(
            fixture.path(),
            &["check-attr", "linguist-vendored", "--", "vendor/library.py"]
        ),
        "vendor/library.py: linguist-vendored: true"
    );
}

#[test]
fn formatting_commit_changes_only_ascii_whitespace() {
    let fixture = RepositoryFixture::build(RepositoryScenario::FormattingOnlyChange)
        .expect("the deterministic repository fixture must build");
    let before = git_output(fixture.path(), &["show", "HEAD^:src/format.ts"]).stdout;
    let after = fs::read(fixture.path().join("src/format.ts"))
        .expect("the formatted source must be readable");

    assert_ne!(before, after);
    assert_eq!(
        without_ascii_whitespace(&before),
        without_ascii_whitespace(&after)
    );
}

#[test]
fn rename_commit_is_detected_as_an_exact_move() {
    let fixture = RepositoryFixture::build(RepositoryScenario::RenameAndMove)
        .expect("the deterministic repository fixture must build");
    assert_eq!(
        git_text(
            fixture.path(),
            &[
                "diff-tree",
                "--no-commit-id",
                "--name-status",
                "-r",
                "-M",
                "HEAD"
            ]
        ),
        "R100\tsrc/legacy.ts\tsrc/core/renamed.ts"
    );
}

#[test]
fn mixed_language_and_missing_community_scenarios_keep_absence_explicit() {
    let mixed = RepositoryFixture::build(RepositoryScenario::SupportedAndUnsupportedLanguages)
        .expect("the deterministic repository fixture must build");
    for path in ["src/main.ts", "src/tool.py", "src/main.rs", "native/tool.c"] {
        assert!(mixed.path().join(path).is_file());
    }

    let missing = RepositoryFixture::build(RepositoryScenario::MissingReadmeAndLicense)
        .expect("the deterministic repository fixture must build");
    assert!(!missing.path().join("README.md").exists());
    assert!(!missing.path().join("LICENSE").exists());
}

fn git_output(repository: &Path, arguments: &[&str]) -> Output {
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

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .expect("the fixture crate must remain under tests/support")
        .to_path_buf()
}

fn git_text(repository: &Path, arguments: &[&str]) -> String {
    let output = git_output(repository, arguments);
    String::from_utf8(output.stdout)
        .expect("fixture Git output must be UTF-8")
        .trim_end()
        .to_owned()
}

fn git_lines(repository: &Path, arguments: &[&str]) -> Vec<String> {
    git_text(repository, arguments)
        .lines()
        .map(str::to_owned)
        .collect()
}

fn expected_timestamp(commit_index: usize) -> &'static str {
    match commit_index {
        0 => "2001-02-03T04:05:06+09:00",
        1 => "2001-02-04T05:06:07+09:00",
        _ => panic!("the initial scenarios use at most two commits"),
    }
}

fn without_ascii_whitespace(bytes: &[u8]) -> Vec<u8> {
    bytes
        .iter()
        .copied()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect()
}

struct ScenarioExpectation {
    scenario: RepositoryScenario,
    commits: Vec<CommitExpectation>,
}

struct CommitExpectation {
    message: &'static str,
    files: BTreeMap<&'static str, &'static [u8]>,
}

fn expectations() -> Vec<ScenarioExpectation> {
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
