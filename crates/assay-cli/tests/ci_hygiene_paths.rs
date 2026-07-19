#![cfg(unix)]
//! Tracked path audit tests for sensitive mutations and public examples.

mod ci_hygiene;

use ci_hygiene::audit::audit_tracked_paths;

use audit::audit_tracked_paths;

#[test]
fn tracked_path_audit_rejects_sensitive_mutations_without_blocking_public_examples() {
    for forbidden in [
        ".env",
        "nested/.env.local",
        ".assay-cache/result.json",
        "config/auth.json",
        "credentials.json",
        "deploy/private.key",
        "tokens/access.token",
        "private-evaluation/run.json",
        "private-evaluations/run.json",
        "private_evaluation_data/run.json",
        "private-evaluations-data/run.json",
        "private-data/run.json",
        "private_datasets/run.json",
        "credentials/provider.json",
        ".credentials/provider.json",
        "credential-store/provider.json",
        ".credential_store/provider.json",
        "credentials_cache/provider.json",
        "aws-credentials/provider.json",
        "auth/session.rs",
        "Auth/session.json",
        ".auth/session.json",
        "auth_store/session.json",
        ".auth-cache/session.json",
        "secrets/examples/value.json",
        "tokens/cache.json",
        ".tokens/cache.json",
        "token-store/cache.json",
        "api-tokens/cache.json",
        "oauth_token_cache/state.json",
        "TOKENS_DATA/cache.json",
        "secrets/value.json",
        ".secrets/value.json",
        "secret_storage/value.json",
        "client_secrets/value.json",
        "live-secrets/value.json",
        "live.secrets/value.json",
        "production.credentials/provider.json",
        "credential-backup/provider.json",
        "token-secrets/value.json",
        ".secret-vault/value.json",
        "credential-store/examples/provider.json",
        "configs/auth/session.json",
        "data/credentials/provider.json",
        "cache/secrets/value.json",
        ".cache/result.json",
        "Node_Modules/package/index.js",
        "dist/bundle.js",
        "build/output.bin",
        ".next/server/app.js",
        "coverage/lcov.info",
        "out/report.json",
        ".turbo/state.json",
        "python/__pycache__/module.pyc",
        ".pytest_cache/state",
        "venv/bin/python",
        ".venv/bin/python",
        "source-clones/repository/README.md",
        "nested/.git/config",
    ] {
        assert!(
            audit_tracked_paths(&[forbidden]).is_err(),
            "sensitive mutation was accepted: {forbidden}"
        );
    }
    audit_tracked_paths(&[
        ".env.example",
        "examples/.env.example",
        "configs/example.toml",
        "tests/fixtures/public-key.example",
        "src/token.rs",
        "crates/assay-identity/src/auth/session.rs",
        "crates/assay-identity/src/api_tokens/policy.rs",
        "web/src/auth/session.ts",
        "docs/auth/session.md",
        "docs/client-secrets/example.md",
        "examples/credentials/provider.json",
        "examples/aws_credentials/provider.json",
        "tests/fixtures/public/tokens/example.json",
        "tests/fixtures/public/oauth-token-cache/state.json",
        "tokenizer/model.json",
        "authentication/session.json",
        "docs/credentials.md",
    ])
    .expect("explicit public examples and ordinary source names stay allowed");
}
