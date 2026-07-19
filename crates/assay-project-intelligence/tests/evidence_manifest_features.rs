use assay_classifier::LinguistAttributeFacts;
use assay_git::CollectionLimits;
use assay_project_intelligence::{
    ClassificationCategoryRecord, ClassifiedSnapshotFile, PortablePathEncoding, RawEvidenceKind,
    assemble_project_evidence, build_project_analysis,
};
use assay_test_fixtures::RepositoryScenario;

mod evidence_manifest_helpers;
use evidence_manifest_helpers::{NamedPolicy, classifications, feature, related_ids, snapshot};

#[test]
fn complete_attribute_facts_allow_absent_features_without_false_unavailability() {
    let snapshot = snapshot(
        RepositoryScenario::MissingReadmeAndLicense,
        CollectionLimits::default(),
    );
    let manifest = assemble_project_evidence(
        &snapshot,
        classifications(&snapshot, LinguistAttributeFacts::available(None, None)),
    )
    .unwrap();
    let output = build_project_analysis(&snapshot, &manifest, "2026-01-02T03:04:06Z").unwrap();
    assert!(
        output["manifest"]["limitations"]
            .as_array()
            .unwrap()
            .iter()
            .all(|limitation| limitation["code"] != "attribute_resolution_unavailable"),
        "complete attribute facts must not claim unavailable resolution"
    );
    let features = output["evidence"].as_array().unwrap();
    for feature in ["readme", "license", "generated_content", "vendored_content"] {
        let fact = features
            .iter()
            .find(|fact| fact["payload"]["feature"] == feature)
            .expect("feature fact must exist");
        assert_eq!(fact["payload"]["state"], "absent");
        assert_eq!(fact["status"], "complete");
    }
}

#[test]
fn repository_features_cite_the_evidence_layer_that_supports_each_claim() {
    let snapshot = snapshot(
        RepositoryScenario::TypeScriptProject,
        CollectionLimits::default(),
    );
    let complete = assemble_project_evidence(
        &snapshot,
        classifications(&snapshot, LinguistAttributeFacts::available(None, None)),
    )
    .unwrap();
    let output = build_project_analysis(&snapshot, &complete, "2026-01-02T03:04:06Z").unwrap();
    let readme_raw_ids = complete
        .raw_facts()
        .iter()
        .filter(|fact| {
            fact.kind() == RawEvidenceKind::TrackedFile
                && fact.source().path().is_some_and(|path| {
                    path.encoding() == PortablePathEncoding::Utf8
                        && path
                            .value()
                            .to_ascii_lowercase()
                            .rsplit('/')
                            .next()
                            .is_some_and(|name| name.starts_with("readme"))
                })
        })
        .map(|fact| fact.id().as_str().to_owned())
        .collect::<Vec<_>>();
    let test_classification_ids = complete
        .classification_facts()
        .iter()
        .filter(|fact| fact.category() == Some(ClassificationCategoryRecord::Test))
        .map(|fact| fact.id().as_str().to_owned())
        .collect::<Vec<_>>();
    let features = output["evidence"].as_array().unwrap();
    let readme = features
        .iter()
        .find(|fact| fact["payload"]["feature"] == "readme")
        .unwrap();
    assert!(!readme_raw_ids.is_empty());
    assert_eq!(related_ids(readme), readme_raw_ids);
    let test_feature = features
        .iter()
        .find(|fact| fact["payload"]["feature"] == "test")
        .unwrap();
    assert_eq!(test_feature["payload"]["state"], "present");
    assert!(!test_classification_ids.is_empty());
    assert_eq!(related_ids(test_feature), test_classification_ids);

    let partial = assemble_project_evidence(
        &snapshot,
        classifications(&snapshot, LinguistAttributeFacts::unavailable()),
    )
    .unwrap();
    let partial_output =
        build_project_analysis(&snapshot, &partial, "2026-01-02T03:04:06Z").unwrap();
    let partial_test_ids = partial
        .classification_facts()
        .iter()
        .filter(|fact| fact.category() == Some(ClassificationCategoryRecord::Test))
        .map(|fact| fact.id().as_str().to_owned())
        .collect::<Vec<_>>();
    let partial_test = partial_output["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .find(|fact| fact["payload"]["feature"] == "test")
        .unwrap();
    assert_eq!(partial_test["payload"]["state"], "unavailable");
    assert!(!partial_test_ids.is_empty());
    assert_eq!(related_ids(partial_test), partial_test_ids);
}

#[test]
fn unavailable_no_match_feature_cites_all_incomplete_classifications_and_policy_identity() {
    let snapshot = snapshot(
        RepositoryScenario::TypeScriptProject,
        CollectionLimits::default(),
    );
    let analyze = |version| {
        let facts = snapshot
            .entries()
            .iter()
            .map(|entry| {
                ClassifiedSnapshotFile::classify(
                    &snapshot,
                    entry,
                    LinguistAttributeFacts::unavailable(),
                    &NamedPolicy(version),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let manifest = assemble_project_evidence(&snapshot, facts).unwrap();
        let expected = manifest
            .classification_facts()
            .iter()
            .map(|fact| fact.id().as_str().to_owned())
            .collect::<Vec<_>>();
        let output = build_project_analysis(&snapshot, &manifest, "2026-01-02T03:04:06Z").unwrap();
        (output, expected)
    };
    let (first, first_expected) = analyze("test-policy-1");
    let (second, second_expected) = analyze("test-policy-2");
    let first_feature = feature(&first, "generated_content");
    let second_feature = feature(&second, "generated_content");
    assert_eq!(first_feature["payload"]["state"], "unavailable");
    assert!(!first_expected.is_empty());
    assert_eq!(related_ids(first_feature), first_expected);
    assert_eq!(related_ids(second_feature), second_expected);
    assert_ne!(first_feature["id"], second_feature["id"]);
    for id in related_ids(first_feature) {
        let cited = first["evidence"]
            .as_array()
            .unwrap()
            .iter()
            .find(|fact| fact["id"] == id)
            .unwrap();
        assert_eq!(cited["payload"]["kind"], "file_classification");
    }
}

#[test]
fn repository_feature_identity_includes_classification_policy_identity() {
    let snapshot = snapshot(
        RepositoryScenario::TypeScriptProject,
        CollectionLimits::default(),
    );
    let analyze = |version| {
        let facts = snapshot
            .entries()
            .iter()
            .map(|entry| {
                ClassifiedSnapshotFile::classify(
                    &snapshot,
                    entry,
                    LinguistAttributeFacts::available(None, None),
                    &NamedPolicy(version),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let manifest = assemble_project_evidence(&snapshot, facts).unwrap();
        build_project_analysis(&snapshot, &manifest, "2026-01-02T03:04:06Z").unwrap()
    };
    let first = analyze("test-policy-1");
    let second = analyze("test-policy-2");
    let feature_id = |value: &serde_json::Value| {
        value["evidence"]
            .as_array()
            .unwrap()
            .iter()
            .find(|fact| fact["payload"]["feature"] == "test")
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    assert_ne!(feature_id(&first), feature_id(&second));
}

#[test]
fn missing_classification_mapper_omits_an_unattempted_policy() {
    let snapshot = snapshot(
        RepositoryScenario::MissingReadmeAndLicense,
        CollectionLimits::default(),
    );
    let manifest = assemble_project_evidence(&snapshot, []).unwrap();
    let output = build_project_analysis(&snapshot, &manifest, "2026-01-02T03:04:06Z").unwrap();
    let missing = output["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .find(|fact| fact["reason"] == "missing_classification")
        .expect("missing classification envelope");
    assert!(missing.get("attempted_policy_version").is_none());
    assert_eq!(missing["requested_kind"], "file_classification");
    assert_eq!(missing["related_evidence_ids"].as_array().unwrap().len(), 1);
}
