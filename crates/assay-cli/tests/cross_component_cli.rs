//! CLI output schema validity and byte-determinism cross-component tests.

mod cross_component;

use cross_component::common;
use cross_component::fixtures;

use std::str::FromStr;

use assay_domain::EvidenceId;
use assay_project_intelligence::validate_project_bundle_consistency;
use assay_test_fixtures::{RepositoryFixture, RepositoryScenario};
use serde_json::Value;

use common::analyze;
use common::assert_valid;
use fixtures::{compile, evidence_bundle, evidence_id, validated_judgments};

#[test]
fn fresh_cli_output_is_schema_valid_bundle_and_byte_deterministic() {
    let fixture = RepositoryFixture::build(RepositoryScenario::TypeScriptProject)
        .expect("fixture must build");

    let first = analyze(fixture.path());
    let second = analyze(fixture.path());
    assert_eq!(
        first, second,
        "the analysis producer must be byte-deterministic under a fixed clock"
    );

    let bundle: Value = serde_json::from_slice(&first).expect("analysis must be JSON");
    assert_valid("project-analysis", &bundle);
    assert_valid("analysis-manifest", &bundle["manifest"]);
    for fact in bundle["evidence"].as_array().expect("evidence array") {
        assert_valid("project-evidence", fact);
    }
    validate_project_bundle_consistency(&bundle)
        .expect("fresh bundle must satisfy cross-component invariants");
    // WIRE-001: the CLI now embeds a project-evaluation instance produced by
    // the deterministic evaluator and score compiler chain.
    assert_valid("project-evaluation", &bundle["evaluation"]);
    assert_eq!(
        bundle["evaluation"]["evaluation_version"],
        "project-intelligence-1"
    );
    // The public numeric Assay Score stays behind the sufficiency gate.
    assert_eq!(
        bundle["evaluation"]["scores"]["assay_score"]["value"],
        Value::Null
    );
}

#[test]
fn cli_evidence_flows_through_evaluator_domain_and_score_compiler() {
    let fixture = RepositoryFixture::build(RepositoryScenario::TypeScriptProject)
        .expect("fixture must build");
    let bundle: Value = serde_json::from_slice(&analyze(fixture.path())).expect("analysis JSON");

    let readme = evidence_id(&bundle, "repository_feature");
    let history = evidence_id(&bundle, "history_scope");
    let snapshot = evidence_id(&bundle, "repository_snapshot");
    let evaluation_ids = [readme.clone(), history, snapshot];

    let evidence_bundle = evidence_bundle(evaluation_ids.clone());
    let validated = validated_judgments(&evidence_bundle);
    let domain_set = validated
        .to_rubric_judgment_set()
        .expect("validated judgments must map onto the domain contract");

    // Every judgment citation is a real identifier the CLI produced.
    let produced: Vec<&EvidenceId> = evaluation_ids.iter().collect();
    for judgment in domain_set.judgments() {
        for citation in judgment.evidence_ids() {
            assert!(
                produced.contains(&citation),
                "a mapped citation must stay inside the fresh CLI evidence set"
            );
        }
    }

    let compiled = compile(domain_set, readme);
    let evaluation = compiled.to_machine_value();
    assert_valid("project-evaluation", &evaluation);
    assert_eq!(evaluation["evaluation_version"], "project-intelligence-1");
    assert_eq!(
        evaluation["compiler"]["judgment_bundle_hash"].as_str(),
        Some(evidence_bundle.content_hash()),
        "the compiled evaluation must record the evaluated bundle hash"
    );
}

#[test]
fn full_chain_evaluation_is_deterministic_and_matches_committed_fixture() {
    // Machine-independent identifiers keep the committed fixture stable across
    // machines while still exercising the evaluator -> domain -> compiler chain.
    let ids = [
        EvidenceId::from_str("evidence:readme:claim-1").unwrap(),
        EvidenceId::from_str("evidence:test:integration-1").unwrap(),
        EvidenceId::from_str("evidence:repository:snapshot").unwrap(),
    ];
    let build = || {
        let bundle = evidence_bundle(ids.clone());
        let judgments = validated_judgments(&bundle)
            .to_rubric_judgment_set()
            .expect("judgments must map onto the domain contract");
        compile(judgments, ids[0].clone()).to_machine_value()
    };

    let evaluation = build();
    assert_valid("project-evaluation", &evaluation);

    let mut serialized = serde_json::to_vec_pretty(&evaluation).expect("evaluation must serialize");
    serialized.push(b'\n');
    let again = {
        let mut bytes = serde_json::to_vec_pretty(&build()).unwrap();
        bytes.push(b'\n');
        bytes
    };
    assert_eq!(
        serialized, again,
        "the full chain must be byte-deterministic"
    );

    let path = common::repository_root().join(common::PRODUCED_EVALUATION);
    if std::env::var_os("ASSAY_BLESS_PRODUCED").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &serialized).unwrap();
    }
    let committed = std::fs::read(&path).unwrap_or_else(|error| {
        panic!(
            "missing produced fixture {}: {error}; regenerate with ASSAY_BLESS_PRODUCED=1",
            path.display()
        )
    });
    assert_eq!(
        committed, serialized,
        "committed producer fixture is stale; regenerate with ASSAY_BLESS_PRODUCED=1"
    );
}
