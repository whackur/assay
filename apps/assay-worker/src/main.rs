use std::{env, process::ExitCode, sync::Arc, time::Duration};

use assay_ai_evaluator::{
    HostedOllamaWorkflowEvaluator, OllamaCompatibleConfig, ProviderSecret, SecretError, SecretName,
    SecretStore,
};
use assay_github::HostedGitHubWorkflowCollector;
use assay_project_intelligence::{
    HostedPortErrorKind, HostedWorkflow, HostedWorkflowOutcome, HostedWorkflowPolicy,
};
use assay_storage::Storage;

#[derive(Clone)]
struct EnvironmentSecretStore;

impl SecretStore for EnvironmentSecretStore {
    fn load(&self, name: &SecretName) -> Result<ProviderSecret, SecretError> {
        env::var(name.as_str())
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .map(ProviderSecret::new)
            .ok_or(SecretError::Unavailable)
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("assay-worker startup failed: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = env::var("DATABASE_URL").map_err(|_| "DATABASE_URL is required")?;
    let storage = Storage::connect(&database_url).await?;
    storage.migrate().await?;

    let config = WorkerConfig::from_env()?;
    let concurrency = bounded_env("ASSAY_WORKER_CONCURRENCY", 3, 1, 3)? as usize;

    // Deps are shared read-only across loops; each loop claims jobs under its own
    // fenced worker id so concurrent claims are safe (see storage SKIP LOCKED).
    let storage = Arc::new(storage);
    let collector = Arc::new(HostedGitHubWorkflowCollector::new(config.github_token));
    let evaluator = Arc::new(HostedOllamaWorkflowEvaluator::new(
        config.ollama,
        EnvironmentSecretStore,
    ));
    let policy = config.policy;
    let pid = std::process::id();

    let mut loops = tokio::task::JoinSet::new();
    for slot in 0..concurrency {
        let storage = Arc::clone(&storage);
        let collector = Arc::clone(&collector);
        let evaluator = Arc::clone(&evaluator);
        let worker_id = format!("worker-{pid}-{slot}");
        loops.spawn(async move {
            worker_loop(&storage, &collector, &evaluator, policy, &worker_id).await;
        });
    }
    while loops.join_next().await.is_some() {}
    Ok(())
}

async fn worker_loop(
    storage: &Storage,
    collector: &HostedGitHubWorkflowCollector,
    evaluator: &HostedOllamaWorkflowEvaluator<EnvironmentSecretStore>,
    policy: HostedWorkflowPolicy,
    worker_id: &str,
) {
    let workflow = HostedWorkflow::new(storage, collector, evaluator, policy);
    loop {
        match workflow.run_once(worker_id).await {
            Ok(HostedWorkflowOutcome::Idle) => tokio::time::sleep(Duration::from_secs(3)).await,
            Ok(HostedWorkflowOutcome::RetryScheduled) => {
                eprintln!("hosted job scheduled for bounded retry");
            }
            Ok(
                HostedWorkflowOutcome::Complete
                | HostedWorkflowOutcome::TerminalUnavailable
                | HostedWorkflowOutcome::LeaseLost,
            ) => {}
            Err(error) if error.kind() == HostedPortErrorKind::Unavailable => {
                eprintln!("hosted workflow dependency unavailable");
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
            Err(_) => tokio::time::sleep(Duration::from_secs(1)).await,
        }
    }
}

struct WorkerConfig {
    github_token: Option<String>,
    ollama: Option<OllamaCompatibleConfig>,
    policy: HostedWorkflowPolicy,
}

impl WorkerConfig {
    fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let github_token = non_empty_env("ASSAY_GITHUB_TOKEN");
        let base = non_empty_env("ASSAY_OLLAMA_BASE_URL");
        let model = non_empty_env("ASSAY_OLLAMA_MODEL");
        let ollama = match (base, model) {
            (Some(base), Some(model)) => {
                let secret_name = non_empty_env("ASSAY_OLLAMA_API_KEY")
                    .map(|_| SecretName::new("ASSAY_OLLAMA_API_KEY"))
                    .transpose()
                    .map_err(|_| "ASSAY_OLLAMA_API_KEY secret name is invalid")?;
                Some(OllamaCompatibleConfig::from_base_url(
                    &base,
                    &model,
                    secret_name,
                )?)
            }
            (None, None) | (Some(_), None) => None,
            (None, Some(_)) => {
                return Err("ASSAY_OLLAMA_BASE_URL is required when a model is configured".into());
            }
        };
        Ok(Self {
            github_token,
            ollama,
            policy: HostedWorkflowPolicy {
                retry_delay_cap_seconds: bounded_env(
                    "ASSAY_PROVIDER_RETRY_MAX_SECONDS",
                    3_600,
                    1,
                    86_400,
                )?,
                failure_circuit_threshold: bounded_env(
                    "ASSAY_PUBLIC_FAILURE_CIRCUIT_THRESHOLD",
                    3,
                    1,
                    100,
                )? as i32,
                failure_circuit_cooldown_seconds: bounded_env(
                    "ASSAY_PUBLIC_FAILURE_CIRCUIT_COOLDOWN_SECONDS",
                    900,
                    10,
                    86_400,
                )?,
            },
        })
    }
}

fn non_empty_env(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn bounded_env(
    name: &str,
    default: i64,
    minimum: i64,
    maximum: i64,
) -> Result<i64, Box<dyn std::error::Error>> {
    let value = non_empty_env(name)
        .map(|value| value.parse::<i64>())
        .transpose()?
        .unwrap_or(default);
    if !(minimum..=maximum).contains(&value) {
        return Err(format!("{name} must be between {minimum} and {maximum}").into());
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_policy_configuration_is_bounded() {
        assert!(bounded_env("ASSAY_TEST_UNSET_BOUND", 8, 1, 10).is_ok());
    }

    #[test]
    fn worker_concurrency_default_is_three() {
        assert_eq!(bounded_env("ASSAY_TEST_UNSET_CONCURRENCY", 3, 1, 3).unwrap(), 3);
    }

    #[test]
    fn worker_concurrency_rejects_out_of_range() {
        assert!(bounded_env("ASSAY_TEST_OVER_CONCURRENCY", 4, 1, 3).is_err());
    }
}
