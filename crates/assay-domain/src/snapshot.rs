use serde::{Deserialize, Serialize};

use crate::identifiers::RevisionId;
use crate::repository::RepositorySource;

/// An immutable repository snapshot used as an analysis input.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceSnapshot {
    source: RepositorySource,
    revision: RevisionId,
    root_tree: Option<RevisionId>,
}

impl SourceSnapshot {
    /// Creates a snapshot pinned to a full revision and optional root tree ID.
    pub const fn new(
        source: RepositorySource,
        revision: RevisionId,
        root_tree: Option<RevisionId>,
    ) -> Self {
        Self {
            source,
            revision,
            root_tree,
        }
    }

    /// Returns the portable repository source.
    pub const fn source(&self) -> &RepositorySource {
        &self.source
    }

    /// Returns the immutable analyzed revision.
    pub const fn revision(&self) -> &RevisionId {
        &self.revision
    }

    /// Returns the immutable root tree ID when it was available.
    pub const fn root_tree(&self) -> Option<&RevisionId> {
        self.root_tree.as_ref()
    }
}
