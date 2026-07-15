use std::process::Command;

use assay_classifier::{
    BuiltInPolicy, ClassificationCategory, ClassificationPolicy, FileClassificationInput,
    LinguistAttributeFacts,
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

fn classify(path: &str, attributes: LinguistAttributeFacts) -> ClassificationCategory {
    let input = FileClassificationInput::try_new(path, attributes)
        .expect("fixture paths must satisfy the portable input contract");
    BuiltInPolicy::V1.classify(&input).category()
}

fn resolved_attributes(repository: &std::path::Path, path: &str) -> LinguistAttributeFacts {
    let output = Command::new("git")
        .current_dir(repository)
        .args([
            "check-attr",
            "linguist-generated",
            "linguist-vendored",
            "--",
            path,
        ])
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
