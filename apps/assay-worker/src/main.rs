use std::{env, process::ExitCode, time::Duration};

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
    let collector = HostedGitHubWorkflowCollector::new(config.github_token);
    let evaluator = HostedOllamaWorkflowEvaluator::new(config.ollama, EnvironmentSecretStore);
    let workflow = HostedWorkflow::new(&storage, &collector, &evaluator, config.policy);
    let worker_id = format!("worker-{}", std::process::id());

    loop {
        match workflow.run_once(&worker_id).await {
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
}
