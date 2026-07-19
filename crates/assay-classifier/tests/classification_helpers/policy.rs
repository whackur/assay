use assay_classifier::{
    AttributeAvailability, BuiltInPolicy, ClassificationCategory, ClassificationDecision,
    ClassificationEvidence, ClassificationPolicy, ClassificationTag, Confidence,
    FileClassificationInput, LinguistAttributeFacts, PolicyVersion, RuleId, classify_with_policy,
};

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
fn false_suppression_tags_survive_crossed_true_primary_overrides() {
    let generated_suppressed = FileClassificationInput::try_new(
        "generated/client.ts",
        LinguistAttributeFacts::available(Some(false), Some(true)),
    )
    .expect("test path must be portable");
    let generated_suppressed = BuiltInPolicy::V1.classify(&generated_suppressed);
    assert_eq!(
        generated_suppressed.category(),
        ClassificationCategory::Vendored
    );
    assert!(
        generated_suppressed
            .tags()
            .contains(&ClassificationTag::GeneratedSuppressed)
    );

    let vendored_suppressed = FileClassificationInput::try_new(
        "vendor/library.py",
        LinguistAttributeFacts::available(Some(true), Some(false)),
    )
    .expect("test path must be portable");
    let vendored_suppressed = BuiltInPolicy::V1.classify(&vendored_suppressed);
    assert_eq!(
        vendored_suppressed.category(),
        ClassificationCategory::Generated
    );
    assert!(
        vendored_suppressed
            .tags()
            .contains(&ClassificationTag::VendoredSuppressed)
    );
}

#[test]
fn false_attributes_do_not_suppress_lower_precedence_rules_under_output_paths() {
    for path in ["coverage/generated/client_pb.js", "build/vendor/library.py"] {
        let input = FileClassificationInput::try_new(
            path,
            LinguistAttributeFacts::available(Some(false), Some(false)),
        )
        .expect("test path must be portable");
        let result = BuiltInPolicy::V1.classify(&input);
        assert!(matches!(
            result.category(),
            ClassificationCategory::Coverage | ClassificationCategory::BuildOutput
        ));
        assert!(
            !result
                .tags()
                .contains(&ClassificationTag::GeneratedSuppressed)
        );
        assert!(
            !result
                .tags()
                .contains(&ClassificationTag::VendoredSuppressed)
        );
    }
}

#[test]
fn false_attributes_remain_evidence_without_claiming_an_unmatched_suppression() {
    let input = FileClassificationInput::try_new(
        "src/main.ts",
        LinguistAttributeFacts::available(Some(false), Some(false)),
    )
    .expect("test path must be portable");
    let result = BuiltInPolicy::V1.classify(&input);

    assert_eq!(result.category(), ClassificationCategory::ProductionCode);
    assert!(
        !result
            .tags()
            .contains(&ClassificationTag::GeneratedSuppressed)
    );
    assert!(
        !result
            .tags()
            .contains(&ClassificationTag::VendoredSuppressed)
    );
    assert!(result.evidence().iter().any(|evidence| {
        evidence.attribute_name() == Some("linguist-generated")
            && evidence.attribute_value() == Some(false)
    }));
    assert!(result.evidence().iter().any(|evidence| {
        evidence.attribute_name() == Some("linguist-vendored")
            && evidence.attribute_value() == Some(false)
    }));
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
fn policy_boundary_can_be_replaced_without_embedding_project_rules() {
    struct DeferredProjectPolicy {
        version: PolicyVersion,
    }

    impl ClassificationPolicy for DeferredProjectPolicy {
        fn policy_version(&self) -> PolicyVersion {
            self.version.clone()
        }

        fn evaluate(&self, _input: &FileClassificationInput) -> ClassificationDecision {
            let rule_id = RuleId::try_new("special_source").expect("static rule ID must be valid");
            ClassificationDecision::new(
                ClassificationCategory::Configuration,
                [],
                rule_id.clone(),
                Confidence::try_from_basis_points(8_000).expect("static confidence must be valid"),
                [ClassificationEvidence::policy_rule(rule_id)],
            )
        }
    }

    let input = FileClassificationInput::try_new(
        "src/main.ts",
        LinguistAttributeFacts::available(None, None),
    )
    .expect("test path must be portable");
    let policy = DeferredProjectPolicy {
        version: PolicyVersion::try_new("deployment-policy-v7")
            .expect("versioned policy identity must be valid"),
    };
    let result = classify_with_policy(&policy, &input);
    assert_eq!(result.category(), ClassificationCategory::Configuration);
    assert_eq!(result.policy_version().as_str(), "deployment-policy-v7");
    assert_eq!(result.rule_id().as_str(), "special_source");
}

#[test]
fn external_policy_results_cannot_drop_linguist_or_unavailable_provenance() {
    struct ExternalPolicy {
        version: PolicyVersion,
    }

    impl ClassificationPolicy for ExternalPolicy {
        fn policy_version(&self) -> PolicyVersion {
            self.version.clone()
        }

        fn evaluate(&self, _input: &FileClassificationInput) -> ClassificationDecision {
            ClassificationDecision::new(
                ClassificationCategory::Configuration,
                [],
                RuleId::try_new("external_special").expect("static rule ID must be valid"),
                Confidence::try_from_basis_points(7_000).expect("static confidence must be valid"),
                [],
            )
        }
    }

    let policy = ExternalPolicy {
        version: PolicyVersion::try_new("external-policy-v7")
            .expect("static policy version must be valid"),
    };
    let false_attributes = FileClassificationInput::try_new(
        "src/main.ts",
        LinguistAttributeFacts::available(Some(false), Some(false)),
    )
    .expect("test path must be portable");
    let false_result = classify_with_policy(&policy, &false_attributes);
    assert_eq!(
        false_result.category(),
        ClassificationCategory::Configuration
    );
    for attribute in ["linguist-generated", "linguist-vendored"] {
        assert!(false_result.evidence().iter().any(|evidence| {
            evidence.attribute_name() == Some(attribute)
                && evidence.attribute_value() == Some(false)
        }));
    }
    assert!(
        !false_result
            .tags()
            .contains(&ClassificationTag::GeneratedSuppressed)
    );
    assert!(
        !false_result
            .tags()
            .contains(&ClassificationTag::VendoredSuppressed)
    );

    let true_attributes = FileClassificationInput::try_new(
        "src/main.ts",
        LinguistAttributeFacts::available(Some(true), Some(true)),
    )
    .expect("test path must be portable");
    let true_result = classify_with_policy(&policy, &true_attributes);
    assert!(
        true_result
            .tags()
            .contains(&ClassificationTag::LinguistGenerated)
    );
    assert!(
        true_result
            .tags()
            .contains(&ClassificationTag::LinguistVendored)
    );
    for attribute in ["linguist-generated", "linguist-vendored"] {
        assert_eq!(
            true_result
                .evidence()
                .iter()
                .filter(|evidence| {
                    evidence.attribute_name() == Some(attribute)
                        && evidence.attribute_value() == Some(true)
                })
                .count(),
            1,
            "canonical attribute evidence must be deduplicated"
        );
    }

    let unavailable =
        FileClassificationInput::try_new("src/main.ts", LinguistAttributeFacts::unavailable())
            .expect("test path must be portable");
    let unavailable_result = classify_with_policy(&policy, &unavailable);
    assert!(
        unavailable_result
            .tags()
            .contains(&ClassificationTag::AttributesUnavailable)
    );
    assert!(
        unavailable_result
            .evidence()
            .iter()
            .any(|evidence| evidence.is_unavailable())
    );
}
