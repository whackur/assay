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
    /// Snapshot pinned to a full revision and optional root tree ID.
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

    /// Portable repository source.
    pub const fn source(&self) -> &RepositorySource {
        &self.source
    }

    /// Immutable analyzed revision.
    pub const fn revision(&self) -> &RevisionId {
        &self.revision
    }

    /// Immutable root tree ID when it was available.
    pub const fn root_tree(&self) -> Option<&RevisionId> {
        self.root_tree.as_ref()
    }
}
