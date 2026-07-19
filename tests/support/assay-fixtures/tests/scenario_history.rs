//! Golden history and identity tests for the deterministic fixture scenarios.

mod common;

use std::fs;

use assay_test_fixtures::{RepositoryFixture, RepositoryFixtureBuilder, RepositoryScenario};

use common::{
    CommitExpectation, ScenarioExpectation, expectations, expected_timestamp, git_lines,
    git_output, git_text,
};

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
        let configured_path = std::path::PathBuf::from(git_text(
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
        common::without_ascii_whitespace(&before),
        common::without_ascii_whitespace(&after)
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

// Suppress unused import warnings for re-exported helper types.
#[allow(dead_code)]
fn _unused_types(_: ScenarioExpectation, _: CommitExpectation) {}
#[allow(dead_code)]
fn _unused_builder(_: RepositoryFixtureBuilder) {}
