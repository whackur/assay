use std::{ffi::OsString, path::PathBuf, time::Duration};

use super::runner::CodexCliRunner;
use super::secret::EnvSecretStore;
use super::workspace::is_commit_hash;
use super::{EVALUATOR_REGISTRY, EvaluatorDescriptor, EvaluatorFamily, resolve_trusted_agent};
use assay_ai_evaluator::SecretError;

#[test]
fn registry_ids_are_stable_and_honest() {
    let ids = EVALUATOR_REGISTRY
        .iter()
        .map(EvaluatorDescriptor::id)
        .collect::<Vec<_>>();
    assert_eq!(ids, ["deterministic", "openai-api-1", "codex-cli-1"]);
    // The deterministic evaluator is wired end to end, so it claims
    // implemented. External AI evaluators remain consent-gated and
    // unimplemented until a live provider is constructed.
    assert!(
        EVALUATOR_REGISTRY
            .iter()
            .filter(|descriptor| descriptor.family() != EvaluatorFamily::Deterministic)
            .all(|descriptor| !descriptor.is_implemented())
    );
    assert!(
        EVALUATOR_REGISTRY
            .iter()
            .find(|descriptor| descriptor.family() == EvaluatorFamily::Deterministic)
            .unwrap()
            .is_implemented()
    );
}

#[test]
fn env_secret_store_maps_values_without_exposing_them() {
    assert!(matches!(
        EnvSecretStore::from_value(None),
        Err(SecretError::NotFound)
    ));
    assert!(matches!(
        EnvSecretStore::from_value(Some(OsString::new())),
        Err(SecretError::NotFound)
    ));
    let secret = EnvSecretStore::from_value(Some(OsString::from("sk-test-value"))).unwrap();
    assert!(!format!("{secret:?}").contains("sk-test-value"));
}

#[test]
fn agent_executable_resolution_requires_an_absolute_operator_path() {
    assert_eq!(resolve_trusted_agent(None), None);
    assert_eq!(resolve_trusted_agent(Some(OsString::new())), None);
    // A bare name would trigger a PATH search; it is untrusted.
    assert_eq!(resolve_trusted_agent(Some(OsString::from("codex"))), None);
    let absolute = std::env::current_exe().expect("test binary path");
    assert_eq!(
        resolve_trusted_agent(Some(absolute.clone().into_os_string())),
        Some(absolute)
    );
}

#[test]
fn runner_rejects_untrusted_executables_and_empty_bounds() {
    assert!(
        CodexCliRunner::from_trusted_executable(
            PathBuf::from("codex"),
            Duration::from_secs(60),
            64 * 1024,
        )
        .is_err()
    );
    let absolute = std::env::current_exe().expect("test binary path");
    assert!(CodexCliRunner::from_trusted_executable(absolute.clone(), Duration::ZERO, 1).is_err());
    assert!(
        CodexCliRunner::from_trusted_executable(absolute, Duration::from_secs(60), 64 * 1024)
            .is_ok()
    );
}

#[test]
fn commit_hash_shape_is_enforced_before_any_command() {
    assert!(is_commit_hash(&"a1".repeat(20)));
    assert!(is_commit_hash(&"b2".repeat(32)));
    assert!(!is_commit_hash("HEAD"));
    assert!(!is_commit_hash("--upload-pack=echo"));
    assert!(!is_commit_hash(&"g".repeat(40)));
}
