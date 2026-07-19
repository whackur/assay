use std::ffi::OsString;

use crate::cli::Evaluator;
use crate::commands::ai_evaluation_capability;
use crate::git::{default_git_candidates, resolve_trusted_git};

#[test]
fn explicit_override_is_authoritative_over_defaults() {
    // Use the running test binary as a stand-in absolute executable so the
    // assertion holds on every platform without a real Git install.
    let executable = std::env::current_exe().expect("test binary path");
    let resolved = resolve_trusted_git(Some(executable.clone().into_os_string()));
    assert_eq!(resolved, Some(executable));
}

#[test]
fn empty_override_falls_back_to_platform_defaults() {
    assert_eq!(
        resolve_trusted_git(Some(OsString::new())),
        resolve_trusted_git(None)
    );
}

#[test]
fn selectable_evaluators_match_the_static_registry_exactly() {
    use clap::ValueEnum;

    let selectable = Evaluator::value_variants()
        .iter()
        .map(|evaluator| evaluator.id())
        .collect::<Vec<_>>();
    let registered = crate::evaluators::EVALUATOR_REGISTRY
        .iter()
        .map(crate::evaluators::EvaluatorDescriptor::id)
        .collect::<Vec<_>>();
    assert_eq!(selectable, registered);
    // The clap value names are the stable registry IDs themselves.
    for evaluator in Evaluator::value_variants() {
        let rendered = evaluator
            .to_possible_value()
            .expect("every evaluator is selectable")
            .get_name()
            .to_owned();
        assert_eq!(rendered, evaluator.id());
    }
}

#[test]
fn ai_evaluation_capability_never_claims_an_unrunnable_evaluator() {
    let feature = ai_evaluation_capability();
    assert_eq!(feature["id"], "ai_evaluation");
    let evaluators = feature["evaluators"].as_array().unwrap();
    assert!(!evaluators.is_empty());
    // The feature may claim implemented only when some evaluator can
    // actually run end to end through this binary.
    let any_implemented = evaluators
        .iter()
        .any(|evaluator| evaluator["status"] == "implemented");
    assert_eq!(
        feature["status"] == "implemented",
        any_implemented,
        "feature status must derive from the per-evaluator statuses"
    );
    // The deterministic evaluator is now wired end to end, so it appears and
    // claims implemented alongside the consent-gated external providers.
    assert!(
        evaluators
            .iter()
            .any(|evaluator| evaluator["id"] == "deterministic")
    );
}

#[test]
fn default_candidates_are_absolute_paths() {
    // The adapter rejects any non-absolute executable as untrusted, so every
    // default candidate must be absolute (ADR 0002 rule 1).
    let candidates = default_git_candidates();
    assert!(!candidates.is_empty());
    assert!(candidates.iter().all(|path| path.is_absolute()));
}
