use std::{env, net::SocketAddr, process::ExitCode};

use assay_storage::{PublicAdmissionLimits, Storage};
use tokio::main as tokio_main;

mod error;
mod handlers;
mod routes;
mod seeding;
mod state;

use state::{AppState, bounded_env, required_env};

#[tokio_main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("assay-api startup failed: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = required_env("DATABASE_URL")?;
    let storage = Storage::connect(&database_url).await?;
    storage.migrate().await?;
    let admission_limits = PublicAdmissionLimits {
        max_active: bounded_env("ASSAY_MAX_ACTIVE_JOBS", 8, 1, 1_000)?,
        completed_cooldown_seconds: bounded_env(
            "ASSAY_PUBLIC_COMPLETED_COOLDOWN_SECONDS",
            14 * 24 * 60 * 60,
            60,
            30 * 24 * 60 * 60,
        )?,
        failure_backoff_seconds: bounded_env(
            "ASSAY_PUBLIC_FAILURE_BACKOFF_SECONDS",
            60,
            10,
            60 * 60,
        )?,
        bucket_window_seconds: bounded_env("ASSAY_PUBLIC_BUCKET_WINDOW_SECONDS", 600, 10, 86_400)?,
        bucket_cooldown_seconds: bounded_env(
            "ASSAY_PUBLIC_BUCKET_COOLDOWN_SECONDS",
            900,
            10,
            86_400,
        )?,
        anonymous_burst: bounded_env("ASSAY_PUBLIC_ANONYMOUS_BURST", 4, 1, 1_000)? as i32,
        owner_burst: bounded_env("ASSAY_PUBLIC_OWNER_BURST", 3, 1, 1_000)? as i32,
        provider_burst: bounded_env("ASSAY_PUBLIC_PROVIDER_BURST", 20, 1, 10_000)? as i32,
    };
    seeding::seed_repositories(&storage, admission_limits.max_active).await;
    let state = AppState {
        storage,
        admission_limits,
    };
    let app = routes::router(state);
    let address: SocketAddr = env::var("ASSAY_API_BIND")
        .unwrap_or_else(|_| "0.0.0.0:8080".to_owned())
        .parse()?;
    let listener = tokio::net::TcpListener::bind(address).await?;
    eprintln!("assay-api listening on {address}");
    axum::serve(listener, app).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admission_configuration_is_bounded() {
        assert!(bounded_env("ASSAY_TEST_UNSET_BOUND", 8, 1, 10).is_ok());
    }
}
