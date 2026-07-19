#![cfg(unix)]
//! Tracked path and staged index audit helpers.

use std::{collections::BTreeSet, path::Path};

pub(crate) fn is_sensitive_data_directory(component: &str) -> bool {
    component
        .to_ascii_lowercase()
        .trim_start_matches('.')
        .split(['.', '-', '_'])
        .filter(|token| !token.is_empty())
        .any(|token| {
            matches!(
                token,
                "auth" | "credential" | "credentials" | "token" | "tokens" | "secret" | "secrets"
            )
        })
}

pub(crate) fn audit_tracked_paths(paths: &[&str]) -> Result<(), String> {
    for path in paths {
        if path.is_empty() || path.starts_with('/') || path.contains('\\') {
            return Err(format!("forbidden tracked path: {path}"));
        }
        let components = path.split('/').collect::<Vec<_>>();
        let normalized_components = components
            .iter()
            .map(|component| component.to_ascii_lowercase().replace('_', "-"))
            .collect::<Vec<_>>();
        if components.iter().any(|component| {
            let normalized = component.to_ascii_lowercase().replace('_', "-");
            matches!(
                normalized.as_str(),
                "" | "."
                    | ".."
                    | ".git"
                    | ".orca"
                    | ".worktrees"
                    | "target"
                    | ".assay-cache"
                    | "source-clones"
                    | "source-cache"
                    | "repository-clones"
                    | "repository-cache"
                    | "private-eval"
                    | "private-evals"
                    | "private-evaluation"
                    | "private-evaluations"
                    | "private-eval-data"
                    | "private-evals-data"
                    | "private-evaluation-data"
                    | "private-evaluations-data"
                    | "private-data"
                    | "private-dataset"
                    | "private-datasets"
                    // Foundation hygiene intentionally forbids checked-in build and cache
                    // directories. Runtime classifier fixtures create these paths only in
                    // temporary repositories, so the product source tree does not need them.
                    | ".cache"
                    | "node-modules"
                    | "dist"
                    | "build"
                    | ".next"
                    | "coverage"
                    | "out"
                    | ".turbo"
                    | "--pycache--"
                    | ".pytest-cache"
                    | ".mypy-cache"
                    | ".ruff-cache"
                    | ".tox"
                    | ".nox"
                    | "venv"
                    | ".venv"
                    | "virtualenv"
                    | ".virtualenv"
            )
        }) {
            return Err(format!("forbidden tracked directory: {path}"));
        }
        let name = components.last().copied().unwrap_or_default();
        if name == ".env.example" {
            continue;
        }
        let lower = name.to_ascii_lowercase();
        let extension = Path::new(&lower)
            .extension()
            .and_then(|value| value.to_str());
        let source_extension = extension.is_some_and(|extension| {
            matches!(extension, "rs" | "ts" | "tsx" | "js" | "jsx" | "py")
        });
        let sensitive_directory_index = components[..components.len() - 1]
            .iter()
            .position(|component| is_sensitive_data_directory(component));
        if let Some(sensitive_index) = sensitive_directory_index {
            let source_artifact = source_extension
                && normalized_components[..sensitive_index]
                    .iter()
                    .any(|component| component == "src");
            let public_context_index = normalized_components
                .iter()
                .position(|component| matches!(component.as_str(), "docs" | "examples"))
                .or_else(|| {
                    normalized_components
                        .windows(2)
                        .position(|pair| pair == ["fixtures", "public"])
                        .map(|index| index + 1)
                });
            let documented_or_public_example =
                public_context_index.is_some_and(|index| index < sensitive_index);
            if !source_artifact && !documented_or_public_example {
                return Err(format!("sensitive tracked data directory: {path}"));
            }
        }
        let sensitive_name = lower == ".env"
            || lower.starts_with(".env.")
            || matches!(
                lower.as_str(),
                "auth.json"
                    | "credentials"
                    | "credentials.json"
                    | "credentials.toml"
                    | "credentials.yaml"
                    | "credentials.yml"
                    | "token"
                    | "token.json"
                    | "token.txt"
                    | "token.toml"
                    | "token.yaml"
                    | "token.yml"
                    | "id_rsa"
                    | "id_ed25519"
                    | "private_key"
                    | "private-key"
                    | "access_token"
                    | "access-token"
                    | "refresh_token"
                    | "refresh-token"
                    | "api_token"
                    | "api-token"
                    | "private-evaluation.json"
            );
        let sensitive_extension = extension.is_some_and(|extension| {
            matches!(
                extension,
                "pem" | "key" | "p12" | "pfx" | "secret" | "token" | "credentials"
            )
        });
        if sensitive_name || sensitive_extension {
            return Err(format!("sensitive tracked file: {path}"));
        }
    }
    Ok(())
}

pub(crate) fn audit_staged_index(output: &[u8]) -> Result<(), String> {
    if output.is_empty() {
        return Ok(());
    }
    if output.last() != Some(&0) {
        return Err("staged index output is not NUL terminated".into());
    }
    let mut paths = Vec::new();
    let mut unique_paths = BTreeSet::new();
    for record in output[..output.len() - 1].split(|byte| *byte == 0) {
        if record.is_empty() {
            return Err("staged index output contains an empty record".into());
        }
        let separator = record
            .iter()
            .position(|byte| *byte == b'\t')
            .ok_or_else(|| "staged index record has no path separator".to_owned())?;
        let metadata = std::str::from_utf8(&record[..separator])
            .map_err(|_| "staged index metadata is not ASCII".to_owned())?;
        let mut fields = metadata.split(' ');
        let mode = fields
            .next()
            .ok_or_else(|| "staged index mode is missing".to_owned())?;
        let object_id = fields
            .next()
            .ok_or_else(|| "staged index object ID is missing".to_owned())?;
        let stage = fields
            .next()
            .ok_or_else(|| "staged index stage is missing".to_owned())?;
        if fields.next().is_some() || mode.is_empty() || object_id.is_empty() || stage.is_empty() {
            return Err("staged index metadata has an invalid field count".into());
        }
        match mode {
            "100644" | "100755" | "120000" => {}
            "160000" => return Err("tracked gitlinks are forbidden".into()),
            _ => return Err("staged index mode is invalid".into()),
        }
        if !matches!(object_id.len(), 40 | 64)
            || !object_id
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            || object_id.bytes().all(|byte| byte == b'0')
        {
            return Err("staged index object ID is invalid".into());
        }
        if stage != "0" {
            return Err("unmerged staged index entries are forbidden".into());
        }
        let path = std::str::from_utf8(&record[separator + 1..])
            .map_err(|_| "tracked path is not UTF-8".to_owned())?;
        if path.is_empty() || !unique_paths.insert(path) {
            return Err("staged index path is empty or duplicated".into());
        }
        paths.push(path);
    }
    audit_tracked_paths(&paths)
}
