//! Adversarial host configuration and Git environment boundary tests.

mod common;

use std::{fs, path::PathBuf};

use assay_test_fixtures::{RepositoryFixture, RepositoryFixtureBuilder, RepositoryScenario};

use common::{git_text, workspace_root};

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

// Suppress unused import warning when `PathBuf` is not referenced on this path.
#[allow(dead_code)]
fn _pathbuf_used(_: PathBuf) {}
