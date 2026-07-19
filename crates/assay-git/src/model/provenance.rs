use crate::GitObjectFormat;

/// Installed Git provenance recorded with every collection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitProvenance {
    adapter_id: &'static str,
    git_version: String,
    object_format: GitObjectFormat,
}

impl GitProvenance {
    pub(crate) fn new(git_version: String, object_format: GitObjectFormat) -> Self {
        Self {
            adapter_id: "installed-git-cli-v1",
            git_version,
            object_format,
        }
    }

    /// Returns the stable adapter identifier.
    pub const fn adapter_id(&self) -> &'static str {
        self.adapter_id
    }

    /// Returns the normalized version reported by the probed executable.
    pub fn git_version(&self) -> &str {
        &self.git_version
    }

    /// Returns the repository object identifier algorithm used by every fact.
    pub const fn object_format(&self) -> GitObjectFormat {
        self.object_format
    }
}
