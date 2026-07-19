use assay_classifier::{
    BUILT_IN_RULE_SET_VERSION, BuiltInPolicy, ClassificationCategory, ClassificationTag,
    FileClassificationInput, LinguistAttributeFacts, PolicyVersion, PortablePath,
};

use super::classify;

#[test]
fn versioned_rules_cover_every_required_primary_category() {
    assert_eq!(
        BuiltInPolicy::V1.version().as_str(),
        BUILT_IN_RULE_SET_VERSION
    );
    assert_eq!(BUILT_IN_RULE_SET_VERSION, "file-classifier-1");
    let cases = [
        ("src/main.ts", ClassificationCategory::ProductionCode),
        ("tests/main.test.ts", ClassificationCategory::Test),
        ("docs/guide.md", ClassificationCategory::Documentation),
        (".github/workflows/ci.yml", ClassificationCategory::CiCd),
        ("infra/main.tf", ClassificationCategory::Infrastructure),
        (
            "db/migrations/001_create_users.sql",
            ClassificationCategory::SchemaMigration,
        ),
        ("package-lock.json", ClassificationCategory::Dependency),
        ("SECURITY.md", ClassificationCategory::SecurityPolicy),
        ("config/app.toml", ClassificationCategory::Configuration),
        ("src/api.generated.ts", ClassificationCategory::Generated),
        ("vendor/library.py", ClassificationCategory::Vendored),
        ("dist/app.js", ClassificationCategory::BuildOutput),
        ("coverage/lcov.info", ClassificationCategory::Coverage),
        ("assets/logo.bin", ClassificationCategory::Unknown),
    ];

    for (path, expected) in cases {
        let result = classify(path);
        assert_eq!(result.category(), expected, "wrong category for {path}");
        assert!(!result.rule_id().as_str().is_empty());
        assert!(result.rule_id().as_str().starts_with("classifier.v1."));
        assert_eq!(result.policy_version().as_str(), BUILT_IN_RULE_SET_VERSION);
        assert!(result.confidence().basis_points() <= 10_000);
        assert!(!result.evidence().is_empty());
        assert_eq!(
            result.attribute_availability(),
            assay_classifier::AttributeAvailability::Available
        );
    }
}

#[test]
fn category_precedence_preserves_specific_work_in_ambiguous_paths() {
    let cases = [
        (
            "tests/generated/client.ts",
            ClassificationCategory::Generated,
        ),
        ("docs/SECURITY.md", ClassificationCategory::SecurityPolicy),
        (
            "tests/migrations/001_schema.sql",
            ClassificationCategory::SchemaMigration,
        ),
        (
            "docs/package-lock.json",
            ClassificationCategory::Documentation,
        ),
        (
            "coverage/vendor/report.json",
            ClassificationCategory::Coverage,
        ),
    ];

    for (path, expected) in cases {
        assert_eq!(
            classify(path).category(),
            expected,
            "wrong precedence for {path}"
        );
    }
}

#[test]
fn dependency_rules_distinguish_manifests_and_lockfiles_with_tags() {
    let manifest = classify("pyproject.toml");
    assert_eq!(manifest.category(), ClassificationCategory::Dependency);
    assert!(
        manifest
            .tags()
            .contains(&ClassificationTag::DependencyManifest)
    );

    let lockfile = classify("pnpm-lock.yaml");
    assert_eq!(lockfile.category(), ClassificationCategory::Dependency);
    assert!(lockfile.tags().contains(&ClassificationTag::Lockfile));

    let bun_lockfile = classify("bun.lock");
    assert_eq!(bun_lockfile.category(), ClassificationCategory::Dependency);
    assert!(bun_lockfile.tags().contains(&ClassificationTag::Lockfile));
}

#[test]
fn protoc_output_rules_are_specific_and_can_be_suppressed() {
    for path in [
        "src/messages_pb.js",
        "src/messages_pb.d.ts",
        "src/messages_pb2.py",
        "src/messages_pb2.pyi",
        "src/messages.pb.go",
    ] {
        assert_eq!(
            classify(path).category(),
            ClassificationCategory::Generated,
            "protoc output was not generated: {path}"
        );
    }

    for path in ["src/pb.js", "src/messages_pb.jsx", "src/messages.pb.json"] {
        assert_ne!(
            classify(path).category(),
            ClassificationCategory::Generated,
            "protobuf rule over-matched: {path}"
        );
    }

    let suppressed = FileClassificationInput::try_new(
        "src/messages_pb.js",
        LinguistAttributeFacts::available(Some(false), None),
    )
    .expect("test path must be portable");
    let suppressed = BuiltInPolicy::V1.classify(&suppressed);
    assert_eq!(
        suppressed.category(),
        ClassificationCategory::ProductionCode
    );
    assert!(
        suppressed
            .tags()
            .contains(&ClassificationTag::GeneratedSuppressed)
    );
}

#[test]
fn unknown_is_preserved_instead_of_becoming_zero_or_production() {
    let result = classify("samples/data.opaque");

    assert_eq!(result.category(), ClassificationCategory::Unknown);
    assert_eq!(result.rule_id().as_str(), "classifier.v1.unknown");
    assert!(result.confidence().basis_points() > 0);
}

#[test]
fn portable_paths_accept_spaces_unicode_and_ascii_case_variants() {
    assert_eq!(
        classify("docs/résumé.MD").category(),
        ClassificationCategory::Documentation
    );
    assert_eq!(
        classify("SRC/hello world.TS").category(),
        ClassificationCategory::ProductionCode
    );
    assert_eq!(
        classify("README.MD").category(),
        ClassificationCategory::Documentation
    );
}

#[test]
fn portable_paths_reject_absolute_traversal_and_nonportable_forms_without_echoing_input() {
    let rejected = [
        "/private/repository/src/main.ts",
        "../secret.ts",
        "src/../../secret.ts",
        "C:/private/repository/main.ts",
        r"src\main.ts",
        "src//main.ts",
        "./src/main.ts",
        "src/\0secret.ts",
        "",
    ];

    for path in rejected {
        let error = PortablePath::try_from(path).expect_err("path must be rejected");
        let diagnostic = format!("{error:?} {error}");
        if !path.is_empty() {
            assert!(
                !diagnostic.contains(path),
                "diagnostic echoed rejected input"
            );
        }
        assert_eq!(error.value_kind(), "portable_path");
    }
}

#[test]
fn error_and_result_debug_output_do_not_expose_paths_or_source_content() {
    let path = "private area/source secret.ts";
    let input =
        FileClassificationInput::try_new(path, LinguistAttributeFacts::available(None, None))
            .expect("test path must be portable");
    let input_debug = format!("{input:?}");
    let result_debug = format!("{:?}", BuiltInPolicy::V1.classify(&input));

    assert!(!input_debug.contains(path));
    assert!(!result_debug.contains(path));
    assert!(!input_debug.contains("source secret"));
    assert!(!result_debug.contains("source secret"));

    let error = PortablePath::try_from("../credential-bearing-name")
        .expect_err("traversal must be rejected");
    let _: assay_classifier::ClassificationError = error.clone();
    assert!(!format!("{error:?} {error}").contains("credential-bearing-name"));
}

#[test]
fn policy_identity_requires_an_explicit_positive_version() {
    for invalid in [
        "deployment-policy",
        "deployment-policy-v0",
        "deployment-policy-0",
        "Deployment-policy-v7",
    ] {
        let error = PolicyVersion::try_new(invalid).expect_err("identity must be rejected");
        assert_eq!(error.value_kind(), "policy_version");
        assert!(!format!("{error:?} {error}").contains(invalid));
    }
}

#[test]
fn policy_identity_rejects_empty_or_ambiguous_separator_segments() {
    for invalid in [
        "deployment--policy-v7",
        "deployment..policy-v7",
        "deployment__policy-v7",
        "deployment._policy-v7",
        "deployment-.policy-v7",
        "deployment_-policy-v7",
        "deployment-policy--v7",
        "deployment-policy.-v7",
    ] {
        let error = PolicyVersion::try_new(invalid).expect_err("identity must be rejected");
        assert_eq!(error.value_kind(), "policy_version");
        assert!(!format!("{error:?} {error}").contains(invalid));
    }
}
