#![cfg(unix)]
//! The foundation vertical slice: schema-valid, private, non-executing analysis.

mod foundation_vertical;

use foundation_vertical::common;
use foundation_vertical::common::{REPOSITORY_EXECUTION_SENTINELS, SECRET_MARKER, repository_root};
use foundation_vertical::fixture::FoundationFixture;
use foundation_vertical::runner::run_analysis;
use foundation_vertical::validation::{audit_bundle_citations, project_analysis_validator};

use std::{collections::BTreeSet, fs};

use serde_json::Value;
use sha2::{Digest, Sha256};

#[test]
fn fixed_repository_is_a_schema_valid_private_and_non_executing_vertical_slice() {
    let fixture = FoundationFixture::build();
    let first = run_analysis(&fixture);
    let second = run_analysis(&fixture);
    assert_eq!(first.status.code(), Some(0));
    assert_eq!(second.status.code(), Some(0));
    assert!(first.stderr.is_empty());
    assert!(second.stderr.is_empty());
    assert_eq!(first.stdout, second.stdout);
    assert!(!first.stdout.windows(2).any(|bytes| bytes == b"\x1b["));
    assert!(
        !fixture.tripwire.exists(),
        "Git filter, textconv, or hook ran"
    );
    for sentinel in REPOSITORY_EXECUTION_SENTINELS {
        assert!(!fixture.repository.join(sentinel).exists());
    }

    let digest = hex::encode(Sha256::digest(&first.stdout));
    let reviewed_digest = fs::read_to_string(
        repository_root().join("tests/golden/cli/foundation-vertical-slice-v1.sha256"),
    )
    .expect("reviewed foundation CLI digest");
    assert_eq!(digest, reviewed_digest.trim());

    let bundle: Value = serde_json::from_slice(&first.stdout).expect("single JSON result");
    audit_bundle_citations(&bundle).expect("closed evidence citations");
    let errors = project_analysis_validator()
        .iter_errors(&bundle)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(errors.is_empty(), "schema errors: {errors:#?}");
    assert_eq!(bundle["schema_version"], "1.0.0");
    let manifest = &bundle["manifest"];
    assert_eq!(manifest["analysis_version"], "repository-evidence-1");
    assert_eq!(
        manifest["rule_set_hash"],
        "sha256:23cc47cd5dc4a4e3f34cfb496daab541461d52d33572c9acc02f14a0cd4a34ae"
    );
    assert_eq!(
        manifest["config_hash"],
        "sha256:bb0850de816d8cb05caf9eda9c593ccc190aeed1873fbdd7d00cb72c18aba92e"
    );
    assert_eq!(manifest["generated_at"], common::FIXED_TIME);
    assert_eq!(manifest["source_snapshot"]["revision"], fixture.revision);
    assert_eq!(manifest["scope"]["head_revision"], fixture.revision);
    assert_eq!(manifest["scope"]["mode"], "single_revision");
    assert_eq!(manifest["status"], "partial");

    let evidence = bundle["evidence"].as_array().expect("evidence array");
    let evidence_ids = evidence
        .iter()
        .map(|record| record["id"].as_str().expect("evidence ID"))
        .collect::<BTreeSet<_>>();
    assert_eq!(evidence_ids.len(), evidence.len());
    let repository_id = manifest["source_snapshot"]["source"]["repository_id"]
        .as_str()
        .expect("portable repository ID");
    assert!(repository_id.starts_with("sha256:"));
    let allowed_statuses = BTreeSet::from([
        "complete",
        "partial",
        "unavailable",
        "unsupported",
        "insufficient",
        "pending",
    ]);
    let mut published_statuses = BTreeSet::new();
    for record in evidence {
        assert_eq!(record["repository"]["repository_id"], repository_id);
        assert_eq!(record["privacy"]["visibility"], "private_local");
        assert_eq!(record["privacy"]["source_content"], "not_retained");
        let status = record["status"].as_str().expect("string evidence status");
        assert!(allowed_statuses.contains(status));
        published_statuses.insert(status);
        if let Some(provenance) = record.get("provenance") {
            assert_eq!(provenance["repository_revision"], fixture.revision);
        }
        for related in record["related_evidence_ids"]
            .as_array()
            .into_iter()
            .flatten()
        {
            assert!(evidence_ids.contains(related.as_str().expect("related evidence ID")));
        }
    }
    assert!(published_statuses.contains("complete"));
    assert!(published_statuses.contains("partial"));
    for source in manifest["data_sources"].as_array().expect("data sources") {
        assert_eq!(source["revision"], fixture.revision);
        assert!(allowed_statuses.contains(source["status"].as_str().expect("data-source status")));
        assert!(evidence_ids.contains(source["id"].as_str().expect("source evidence ID")));
    }
    assert!(
        allowed_statuses.contains(
            manifest["scope"]["history_status"]
                .as_str()
                .expect("history status")
        )
    );

    let raw_ids = evidence
        .iter()
        .filter(|record| record["payload"]["kind"] == "tracked_file")
        .map(|record| record["id"].as_str().expect("raw ID"))
        .collect::<BTreeSet<_>>();
    let classifications = evidence
        .iter()
        .filter(|record| record["payload"]["kind"] == "file_classification")
        .collect::<Vec<_>>();
    assert_eq!(raw_ids.len(), 21);
    assert_eq!(classifications.len(), raw_ids.len());
    assert!(classifications.iter().all(|record| {
        record["payload"]["source_evidence_id"]
            .as_str()
            .is_some_and(|id| raw_ids.contains(id))
            && record["attempted_policy_version"] == "file-classifier-1"
    }));
    let categories = classifications
        .iter()
        .filter_map(|record| record["payload"]["classification"]["primary_category"].as_str())
        .collect::<BTreeSet<_>>();
    let expected_categories = BTreeSet::from([
        "build_output",
        "ci_cd",
        "configuration",
        "coverage",
        "dependency",
        "documentation",
        "generated",
        "infrastructure",
        "production_code",
        "schema_migration",
        "security",
        "test",
        "vendored",
    ]);
    assert!(
        expected_categories.is_subset(&categories),
        "categories: {categories:#?}"
    );
    let unsupported_language = evidence.iter().find(|record| {
        record["payload"]["kind"] == "tracked_file"
            && record["payload"]["path"]["value"] == "native/unsupported.rs"
            && record["payload"]["language_status"] == "unsupported"
    });
    assert!(unsupported_language.is_some());
    let unavailable_feature = evidence.iter().find(|record| {
        record["payload"]["kind"] == "repository_feature"
            && record["payload"]["state"] == "unavailable"
            && record["payload"]["related_evidence_ids"]
                .as_array()
                .is_some_and(|ids| !ids.is_empty())
    });
    assert!(unavailable_feature.is_some());

    let limitations = manifest["limitations"]
        .as_array()
        .expect("manifest limitations");
    for code in [
        "attribute_resolution_unavailable",
        "project_scores_not_computed",
        "repository_code_not_executed",
    ] {
        assert!(limitations.iter().any(|item| item["code"] == code));
    }
    let text = String::from_utf8(first.stdout.clone()).expect("UTF-8 JSON");
    for forbidden in [
        fixture.repository.to_string_lossy().as_ref(),
        fixture.tripwire.to_string_lossy().as_ref(),
        SECRET_MARKER,
        "private-source-body",
        "foundation-fixture@example.invalid",
        "raw_diff",
        "person_score",
    ] {
        assert!(
            !text.contains(forbidden),
            "published forbidden value: {forbidden}"
        );
    }
    // The public numeric Assay Score stays behind the sufficiency and
    // calibration gates: the field is present but its value is null while
    // essential dimensions remain unscored.
    let bundle: Value = serde_json::from_slice(&first.stdout).expect("analysis bundle");
    assert_eq!(
        bundle["evaluation"]["scores"]["assay_score"]["value"],
        Value::Null
    );
    assert_eq!(
        bundle["evaluation"]["scores"]["assay_score"]["status"],
        "insufficient"
    );
}
