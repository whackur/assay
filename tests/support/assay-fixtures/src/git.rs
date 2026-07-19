//! Deployment-trusted Git executable resolution for integration tests.

use std::{env, path::PathBuf};

/// Environment variable that names one trusted, absolute Git executable.
///
/// This mirrors the CLI's trusted deployment or startup configuration input
/// (ADR 0002 rule 1) so integration tests resolve the same executable as the
/// product on every platform.
pub const GIT_EXECUTABLE_ENV: &str = "ASSAY_GIT_EXECUTABLE";

/// Resolves a deployment-trusted absolute Git executable for integration
/// tests, mirroring the CLI's resolution contract: a non-empty
/// [`GIT_EXECUTABLE_ENV`] override is authoritative, otherwise the first
/// well-known absolute install location that exists on this platform wins.
pub fn trusted_git_executable() -> Option<PathBuf> {
    if let Some(value) = env::var_os(GIT_EXECUTABLE_ENV)
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
fn default_git_candidates() -> Vec<PathBuf> {
    ["/usr/bin/git", "/usr/local/bin/git"]
        .into_iter()
        .map(PathBuf::from)
        .collect()
}

/// Well-known absolute install locations for Git for Windows, derived from the
/// trusted `Program Files` startup environment with fixed fallbacks. Custom
/// installs are supported through [`GIT_EXECUTABLE_ENV`].
#[cfg(windows)]
fn default_git_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    for key in ["ProgramW6432", "ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(base) = env::var_os(key)
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
