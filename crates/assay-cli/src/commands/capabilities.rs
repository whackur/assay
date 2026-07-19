use serde_json::{Value, json};

use crate::evaluators;

pub(crate) fn capabilities() -> Value {
    json!({
        "schema_version": "1.0.0",
        "tool": { "name": "assay", "version": env!("CARGO_PKG_VERSION") },
        "commands": ["capabilities", "history", "project analyze", "serve"],
        "formats": ["json"],
        "schemas": [
            { "name": "analysis-manifest", "version": "1.0.0" },
            { "name": "capabilities", "version": "1.0.0" },
            { "name": "project-analysis", "version": "1.0.0" },
            { "name": "project-evidence", "version": "1.0.0" },
            { "name": "project-evaluation", "version": "1.0.0" }
        ],
        "languages": ["javascript", "python", "tsx", "typescript"],
        "features": [
            ai_evaluation_capability(),
            { "id": "attribute_resolution", "status": "not_implemented" },
            { "id": "file_classification", "status": "implemented" },
            { "id": "github_collection", "status": "not_implemented" },
            { "id": "local_git_snapshot", "status": "implemented" },
            { "id": "local_private_history", "status": "implemented" },
            { "id": "loopback_dashboard", "status": "implemented" },
            { "id": "project_scores", "status": "implemented" },
            { "id": "remote_private_fetch", "status": "not_implemented" },
            { "id": "repository_code_execution", "status": "prohibited" },
            { "id": "semantic_diff", "status": "not_implemented" }
        ]
    })
}

/// Reports the `ai_evaluation` feature honestly from the static evaluator
/// registry (ADR 0012): every registered evaluator ID is listed with its
/// family and its per-binary status, and the feature claims `implemented`
/// only when at least one evaluator can actually run end to end. The
/// deterministic default performs rubric evaluation locally without network,
/// so it counts toward `implemented` once the CLI wires it through to a
/// validated judgment set.
pub(crate) fn ai_evaluation_capability() -> Value {
    let evaluators = evaluators::EVALUATOR_REGISTRY
        .iter()
        .map(|descriptor| {
            json!({
                "id": descriptor.id(),
                "family": descriptor.family().code(),
                "status": if descriptor.is_implemented() { "implemented" } else { "not_implemented" }
            })
        })
        .collect::<Vec<_>>();
    let implemented = evaluators
        .iter()
        .any(|evaluator| evaluator["status"] == "implemented");
    json!({
        "id": "ai_evaluation",
        "status": if implemented { "implemented" } else { "not_implemented" },
        "evaluators": evaluators
    })
}
