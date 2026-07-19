mod schema_contracts_helpers;
use serde_json::Value;

use schema_contracts_helpers::common::{assert_rejected, read_json, repository_root, validator};

#[test]
fn analysis_manifest_requires_effective_config_and_component_provenance() {
    let validator = validator("analysis-manifest");
    let golden = read_json(repository_root().join("tests/golden/analysis-manifest-v1.json"));

    for required in ["config_hash", "analyzers", "parsers"] {
        let mut missing = golden.clone();
        missing
            .as_object_mut()
            .expect("manifest must be an object")
            .remove(required);
        assert_rejected(
            &validator,
            &missing,
            "analysis-manifest",
            &format!("missing {required}"),
        );
    }

    let analyzers = golden["analyzers"]
        .as_array()
        .expect("golden analyzers must be an array");
    assert!(!analyzers.is_empty(), "at least one analyzer is required");
    let mut sorted_analyzers = analyzers.clone();
    sorted_analyzers.sort_by(|left, right| {
        left["name"]
            .as_str()
            .cmp(&right["name"].as_str())
            .then_with(|| left["version"].as_str().cmp(&right["version"].as_str()))
    });
    assert_eq!(analyzers, &sorted_analyzers, "analyzers must be sorted");

    let mut duplicate_analyzer = golden.clone();
    duplicate_analyzer["analyzers"] =
        serde_json::json!([analyzers[0].clone(), analyzers[0].clone()]);
    assert_rejected(
        &validator,
        &duplicate_analyzer,
        "analysis-manifest",
        "duplicate analyzer component",
    );

    let mut duplicate_parser = golden;
    duplicate_parser["parsers"] = serde_json::json!([
        {"name": "tree-sitter-typescript", "version": "0.23.2"},
        {"name": "tree-sitter-typescript", "version": "0.23.2"}
    ]);
    assert_rejected(
        &validator,
        &duplicate_parser,
        "analysis-manifest",
        "duplicate parser component",
    );
}

#[test]
fn public_revision_contracts_reject_git_null_object_ids() {
    for null_oid in ["0".repeat(40), "0".repeat(64)] {
        for (contract, pointer) in [
            ("analysis-manifest", "/source_snapshot/revision"),
            ("project-evidence", "/provenance/repository_revision"),
            ("project-evaluation", "/project/revision"),
        ] {
            schema_contracts_helpers::assertions::assert_golden_value_rejected(
                contract,
                pointer,
                Value::String(null_oid.clone()),
                "null revision",
            );
        }
    }
}

#[test]
fn rfc3339_utc_fields_require_semantically_valid_dates() {
    for (contract, pointer) in [
        ("analysis-manifest", "/generated_at"),
        ("project-evidence", "/provenance/collected_at"),
    ] {
        schema_contracts_helpers::assertions::assert_golden_value_rejected(
            contract,
            pointer,
            Value::String("2026-13-40T25:61:61Z".to_owned()),
            "malformed date-time",
        );
    }
}

#[test]
fn remote_record_ids_are_portable_non_path_identifiers() {
    for unsafe_id in [
        "/records/1",
        "C:/records/1",
        "records\\1",
        "records/../1",
        "records/./1",
    ] {
        for (contract, pointer) in [
            ("analysis-manifest", "/data_sources/0/remote_record_id"),
            ("project-evidence", "/provenance/remote_record_id"),
        ] {
            schema_contracts_helpers::assertions::assert_golden_value_rejected(
                contract,
                pointer,
                Value::String(unsafe_id.to_owned()),
                "unsafe remote record identifier",
            );
        }
    }
}

#[test]
fn privacy_citation_and_product_domain_boundaries_are_enforced() {
    for unsafe_path in [
        "/private/source.rs",
        "../source.rs",
        "C:\\private\\source.rs",
    ] {
        schema_contracts_helpers::assertions::assert_golden_value_rejected(
            "project-evidence",
            "/payload/relative_path",
            Value::String(unsafe_path.to_owned()),
            "non-portable source path",
        );
    }
    schema_contracts_helpers::assertions::assert_golden_value_rejected(
        "project-evidence",
        "/privacy/visibility",
        Value::String("private_local".to_owned()),
        "private evidence without an explicit transmission boundary",
    );

    schema_contracts_helpers::assertions::assert_golden_value_rejected(
        "ai-judgment",
        "/judgments/0/evidence_ids",
        Value::Array(Vec::new()),
        "uncited applicable judgment",
    );
    schema_contracts_helpers::assertions::assert_golden_value_rejected(
        "ai-judgment",
        "/judgments/0/rating",
        Value::Number(5.into()),
        "out-of-range rating",
    );

    schema_contracts_helpers::assertions::assert_golden_mutation_rejected(
        "project-evaluation",
        "person-level score",
        |instance| {
            instance["scores"]
                .as_object_mut()
                .expect("scores must be an object")
                .insert("person_performance".to_owned(), Value::Null);
        },
    );
    schema_contracts_helpers::assertions::assert_golden_mutation_rejected(
        "project-evaluation",
        "numeric score without evidence citations",
        |instance| {
            instance["scores"]["project_substance"]["status"] =
                Value::String("complete".to_owned());
            instance["scores"]["project_substance"]["value"] = Value::Number(80.into());
        },
    );
    schema_contracts_helpers::assertions::assert_golden_value_rejected(
        "analysis-manifest",
        "/status",
        Value::String("complete".to_owned()),
        "complete run with partial evidence",
    );
}
