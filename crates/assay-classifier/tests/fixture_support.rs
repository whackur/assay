use std::{
    env,
    ffi::{OsStr, OsString},
    process::Command,
};

use assay_classifier::{
    BuiltInPolicy, ClassificationCategory, FileClassificationInput, LinguistAttributeFacts,
};
use assay_test_fixtures::{RepositoryFixture, RepositoryScenario};

#[test]
fn classifier_consumes_resolved_linguist_attributes_from_the_real_fixture() {
    let fixture = RepositoryFixture::build(RepositoryScenario::GeneratedAndVendoredOverrides)
        .expect("the attributes fixture must build");

    let generated = resolved_attributes(fixture.path(), "generated/client.ts");
    let vendored = resolved_attributes(fixture.path(), "vendor/library.py");
    let production = resolved_attributes(fixture.path(), "src/main.ts");

    assert_eq!(
        classify("generated/client.ts", generated),
        ClassificationCategory::Generated
    );
    assert_eq!(
        classify("vendor/library.py", vendored),
        ClassificationCategory::Vendored
    );
    assert_eq!(
        classify("src/main.ts", production),
        ClassificationCategory::ProductionCode
    );
}

#[test]
fn git_attribute_query_ignores_hostile_inherited_git_environment() {
    let fixture = RepositoryFixture::build(RepositoryScenario::GeneratedAndVendoredOverrides)
        .expect("the attributes fixture must build");
    let redirected = fixture.path().join("hostile git metadata");
    let environment = [
        (
            OsString::from("GIT_DIR"),
            redirected.clone().into_os_string(),
        ),
        (
            OsString::from("GIT_WORK_TREE"),
            redirected.clone().into_os_string(),
        ),
        (
            OsString::from("GIT_INDEX_FILE"),
            redirected.join("index").into_os_string(),
        ),
        (
            OsString::from("GIT_CONFIG_GLOBAL"),
            redirected.join("config").into_os_string(),
        ),
        (OsString::from("GIT_CONFIG_NOSYSTEM"), OsString::from("0")),
        (OsString::from("GIT_TERMINAL_PROMPT"), OsString::from("1")),
        (OsString::from("GIT_PAGER"), OsString::from("hostile-pager")),
        (OsString::from("PAGER"), OsString::from("hostile-pager")),
    ];

    let attributes =
        resolved_attributes_with_environment(fixture.path(), "generated/client.ts", &environment);
    assert_eq!(attributes.generated(), Some(true));
    assert_eq!(attributes.vendored(), None);
    assert!(!redirected.exists());
}

fn classify(path: &str, attributes: LinguistAttributeFacts) -> ClassificationCategory {
    let input = FileClassificationInput::try_new(path, attributes)
        .expect("fixture paths must satisfy the portable input contract");
    BuiltInPolicy::V1.classify(&input).category()
}

fn resolved_attributes(repository: &std::path::Path, path: &str) -> LinguistAttributeFacts {
    resolved_attributes_with_environment(repository, path, &[])
}

fn resolved_attributes_with_environment(
    repository: &std::path::Path,
    path: &str,
    inherited_environment: &[(OsString, OsString)],
) -> LinguistAttributeFacts {
    let mut command = Command::new("git");
    command
        .current_dir(repository)
        .envs(inherited_environment.iter().cloned())
        .args([
            "check-attr",
            "linguist-generated",
            "linguist-vendored",
            "--",
            path,
        ]);
    remove_git_environment(&mut command, inherited_environment);
    let isolated_global_config = repository
        .parent()
        .expect("fixture repository must have an isolated parent")
        .join("empty-git-config");
    command
        .env("GIT_ATTR_NOSYSTEM", "1")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", isolated_global_config)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_PAGER", "cat")
        .env("PAGER", "cat")
        .env("LC_ALL", "C")
        .env("TZ", "UTC");
    let output = command
        .output()
        .expect("Git must be available for fixture policy resolution");
    assert!(output.status.success(), "Git attribute query must succeed");

    let stdout = String::from_utf8(output.stdout).expect("fixture attribute output must be UTF-8");
    let mut generated = None;
    let mut vendored = None;
    for line in stdout.lines() {
        let mut fields = line.rsplitn(2, ": ");
        let value = fields
            .next()
            .expect("attribute output must contain a value");
        let prefix = fields.next().expect("attribute output must contain a name");
        let attribute = prefix
            .rsplit_once(": ")
            .map(|(_, attribute)| attribute)
            .expect("attribute output must contain a path and name");
        let parsed = match value {
            "set" | "true" => Some(true),
            "unset" | "false" => Some(false),
            "unspecified" => None,
            _ => panic!("unexpected synthetic attribute representation"),
        };
        match attribute {
            "linguist-generated" => generated = parsed,
            "linguist-vendored" => vendored = parsed,
            _ => panic!("unexpected synthetic attribute name"),
        }
    }

    LinguistAttributeFacts::available(generated, vendored)
}

fn remove_git_environment(command: &mut Command, injected_environment: &[(OsString, OsString)]) {
    for (key, _) in env::vars_os() {
        if is_git_environment_key(&key) {
            command.env_remove(key);
        }
    }
    for (key, _) in injected_environment {
        if is_git_environment_key(key) {
            command.env_remove(key);
        }
    }
    for key in ["PAGER", "LESS", "LV", "PROMPT_COMMAND", "SSH_ASKPASS"] {
        command.env_remove(key);
    }
}

fn is_git_environment_key(key: &OsStr) -> bool {
    key.to_string_lossy()
        .get(..4)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("GIT_"))
}
