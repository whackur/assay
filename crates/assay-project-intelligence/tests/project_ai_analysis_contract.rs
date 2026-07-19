use assay_project_intelligence::{ProjectAiAnalysisEnvelope, StoredProjectAiAnalysis};
use jsonschema::Draft;
use serde_json::Value;

#[test]
fn golden_public_project_ai_analysis_matches_closed_schema() {
    let schema: Value =
        serde_json::from_str(include_str!("../../../schemas/project-ai-analysis/v1.json")).unwrap();
    let validator = jsonschema::options()
        .with_draft(Draft::Draft202012)
        .build(&schema)
        .unwrap();
    let golden: Value = serde_json::from_str(include_str!(
        "../../../schemas/project-ai-analysis/v1.golden.json"
    ))
    .unwrap();
    assert!(validator.is_valid(&golden));

    let document = serde_json::json!({
        "schema_version":"1.0.0", "evaluation_version":"hosted-evaluation-1",
        "rubric_version":"project-rubric-1", "status":"complete",
        "evidence_bundle_hash":"sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "privacy":{"evidence_scope":"public_only","external_transmission":"not_used"},
        "judgments":[{"criterion_id":"quality.documentation","applicability":"applicable","rating":3,"rating_scale":4,"confidence":0.8,"evidence_ids":["evidence:github:readme"],"rationale":"The public README provides clear setup and usage guidance."}]
    });
    let output = ProjectAiAnalysisEnvelope::from_stored(StoredProjectAiAnalysis {
        owner: "assay".into(),
        repository: "example".into(),
        commit_sha: "0123456789abcdef0123456789abcdef01234567".into(),
        default_branch: "main".into(),
        evaluation_version: "hosted-evaluation-1".into(),
        rubric_version: "project-rubric-1".into(),
        evidence_bundle_hash:
            "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
        judgment: document,
    })
    .unwrap();
    assert!(validator.is_valid(&serde_json::to_value(output).unwrap()));
}
