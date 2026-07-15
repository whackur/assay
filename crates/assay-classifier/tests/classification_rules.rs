use assay_classifier::{
    AttributeAvailability, BUILT_IN_RULE_SET_VERSION, BuiltInPolicy, ClassificationCategory,
    ClassificationError, ClassificationEvidence, ClassificationPolicy, ClassificationTag,
    Confidence, FileClassification, FileClassificationInput, LinguistAttributeFacts, PortablePath,
    RuleId,
};

fn classify(path: &str) -> assay_classifier::FileClassification {
    let input =
        FileClassificationInput::try_new(path, LinguistAttributeFacts::available(None, None))
            .expect("test paths must be portable");
    BuiltInPolicy::V1.classify(&input)
}

#[test]
fn versioned_rules_cover_every_required_primary_category() {
    assert_eq!(BuiltInPolicy::V1.version(), BUILT_IN_RULE_SET_VERSION);
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
        assert!(result.confidence().basis_points() <= 10_000);
        assert!(!result.evidence().is_empty());
        assert_eq!(
            result.attribute_availability(),
            AttributeAvailability::Available
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
}

#[test]
fn linguist_true_overrides_take_precedence_and_retain_provenance() {
    let input = FileClassificationInput::try_new(
        "src/ordinary.ts",
        LinguistAttributeFacts::available(Some(true), Some(true)),
    )
    .expect("test path must be portable");
    let result = BuiltInPolicy::V1.classify(&input);

    assert_eq!(result.category(), ClassificationCategory::Generated);
    assert!(
        result
            .tags()
            .contains(&ClassificationTag::LinguistGenerated)
    );
    assert!(result.tags().contains(&ClassificationTag::LinguistVendored));
    assert_eq!(
        result.rule_id().as_str(),
        "classifier.v1.attribute.generated"
    );
    assert!(result.evidence().iter().any(|evidence| {
        evidence.attribute_name() == Some("linguist-generated")
            && evidence.attribute_value() == Some(true)
    }));
    assert!(result.evidence().iter().any(|evidence| {
        evidence.attribute_name() == Some("linguist-vendored")
            && evidence.attribute_value() == Some(true)
    }));
}

#[test]
fn linguist_false_overrides_disable_matching_builtin_generated_and_vendor_rules() {
    let generated = FileClassificationInput::try_new(
        "generated/client.ts",
        LinguistAttributeFacts::available(Some(false), None),
    )
    .expect("test path must be portable");
    let generated = BuiltInPolicy::V1.classify(&generated);
    assert_eq!(generated.category(), ClassificationCategory::ProductionCode);
    assert!(
        generated
            .tags()
            .contains(&ClassificationTag::GeneratedSuppressed)
    );

    let vendored = FileClassificationInput::try_new(
        "vendor/library.py",
        LinguistAttributeFacts::available(None, Some(false)),
    )
    .expect("test path must be portable");
    let vendored = BuiltInPolicy::V1.classify(&vendored);
    assert_eq!(vendored.category(), ClassificationCategory::ProductionCode);
    assert!(
        vendored
            .tags()
            .contains(&ClassificationTag::VendoredSuppressed)
    );
}

#[test]
fn unavailable_attribute_facts_remain_explicit_and_do_not_become_false() {
    let input =
        FileClassificationInput::try_new("src/main.ts", LinguistAttributeFacts::unavailable())
            .expect("test path must be portable");
    let result = BuiltInPolicy::V1.classify(&input);

    assert_eq!(result.category(), ClassificationCategory::ProductionCode);
    assert_eq!(
        result.attribute_availability(),
        AttributeAvailability::Unavailable
    );
    assert!(
        result
            .tags()
            .contains(&ClassificationTag::AttributesUnavailable)
    );
    assert!(
        result
            .evidence()
            .iter()
            .any(|evidence| evidence.is_unavailable())
    );
    assert!(result.confidence().basis_points() <= 7_500);
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
    let _: ClassificationError = error.clone();
    assert!(!format!("{error:?} {error}").contains("credential-bearing-name"));
}

#[test]
fn policy_boundary_can_be_replaced_without_embedding_project_rules() {
    struct DeferredProjectPolicy;

    impl ClassificationPolicy for DeferredProjectPolicy {
        fn classify(&self, input: &FileClassificationInput) -> FileClassification {
            let rule_id = RuleId::try_new("deployment-policy.v7.special_source")
                .expect("static rule ID must be valid");
            FileClassification::from_policy(
                ClassificationCategory::Configuration,
                [],
                rule_id.clone(),
                Confidence::try_from_basis_points(8_000).expect("static confidence must be valid"),
                [ClassificationEvidence::policy_rule(rule_id)],
                input.attributes().availability(),
            )
        }
    }

    let input = FileClassificationInput::try_new(
        "src/main.ts",
        LinguistAttributeFacts::available(None, None),
    )
    .expect("test path must be portable");
    assert_eq!(
        DeferredProjectPolicy.classify(&input).category(),
        ClassificationCategory::Configuration
    );
}
