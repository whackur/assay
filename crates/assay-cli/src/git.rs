use std::{ffi::OsString, path::PathBuf};

use assay_git::CollectionLimits;

use crate::errors::{RunError, invalid_test_limit};

/// Environment variable that names one trusted, absolute Git executable.
///
/// This is a trusted deployment or startup configuration input per ADR 0002
/// rule 1. It is never derived from repository content and lets operators use
/// a non-default install location on any platform.
pub const GIT_EXECUTABLE_ENV: &str = "ASSAY_GIT_EXECUTABLE";

/// Resolves the Git executable from trusted deployment configuration or a
/// trusted startup environment, never from repository content (ADR 0002).
pub(crate) fn trusted_git() -> Option<PathBuf> {
    resolve_trusted_git(std::env::var_os(GIT_EXECUTABLE_ENV))
}

/// Pure resolution used by [`trusted_git`], split out so the precedence and
/// absolute-path contract can be tested without mutating the process
/// environment. An explicit override is authoritative; the adapter still
/// probes it and reports an explicit error if it is not a compatible Git.
pub(crate) fn resolve_trusted_git(override_value: Option<OsString>) -> Option<PathBuf> {
    if let Some(value) = override_value
        && !value.is_empty()
    {
        return Some(PathBuf::from(value));
    }
    default_git_candidates()
        .into_iter()
        .find(|path| path.is_file())
}

/// Well-known absolute install locations for a supported Git on Unix.
#[cfg(unix)]
pub(crate) fn default_git_candidates() -> Vec<PathBuf> {
    ["/usr/bin/git", "/usr/local/bin/git"]
        .into_iter()
        .map(PathBuf::from)
        .collect()
}

/// Well-known absolute install locations for Git for Windows, derived from the
/// trusted `Program Files` startup environment with fixed fallbacks. Custom
/// installs are supported through [`GIT_EXECUTABLE_ENV`].
#[cfg(windows)]
pub(crate) fn default_git_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    for key in ["ProgramW6432", "ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(base) = std::env::var_os(key)
            && !base.is_empty()
        {
            let base = PathBuf::from(base);
            candidates.push(base.join(r"Git\cmd\git.exe"));
            candidates.push(base.join(r"Git\bin\git.exe"));
        }
    }
    candidates.push(PathBuf::from(r"C:\Program Files\Git\cmd\git.exe"));
    candidates.push(PathBuf::from(r"C:\Program Files\Git\bin\git.exe"));
    candidates
}

pub(crate) fn collection_limits() -> Result<CollectionLimits, RunError> {
    let mut limits = CollectionLimits::default();
    if cfg!(debug_assertions) {
        if let Some(value) = std::env::var_os("ASSAY_TEST_MAX_OBJECT_BYTES") {
            limits.max_object_bytes = value
                .to_str()
                .and_then(|v| v.parse().ok())
                .filter(|v| *v > 0)
                .ok_or_else(invalid_test_limit)?;
        }
        if let Some(value) = std::env::var_os("ASSAY_TEST_MAX_HISTORY_COMMITS") {
            limits.max_history_commits = value
                .to_str()
                .and_then(|v| v.parse().ok())
                .filter(|v| *v > 0)
                .ok_or_else(invalid_test_limit)?;
        }
    }
    Ok(limits)
}
