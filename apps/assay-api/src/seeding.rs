use std::env;

use assay_github::CanonicalGitHubRepository;
use assay_storage::Storage;

pub(crate) async fn seed_repositories(storage: &Storage, max_active_jobs: i64) {
    let seeds = env::var("ASSAY_SEED_REPOSITORIES").unwrap_or_else(|_| "whackur/assay".to_owned());
    for raw in seeds
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        match CanonicalGitHubRepository::parse(raw) {
            Ok(repository) => {
                if let Err(error) = storage
                    .submit_seed(repository.owner(), repository.name(), max_active_jobs)
                    .await
                {
                    eprintln!(
                        "seed admission failed for {}: {error}",
                        repository.identifier()
                    );
                }
            }
            Err(_) => eprintln!("ignored invalid ASSAY_SEED_REPOSITORIES entry"),
        }
    }
}
