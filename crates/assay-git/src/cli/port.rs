use std::str::FromStr;

use assay_domain::{EvidenceStatus, RevisionId, SourceSnapshot};

use crate::{
    CollectionError, CollectionErrorKind, CollectionStage, GitProvenance, RepositorySnapshot,
    RepositorySnapshotPort, SnapshotRequest, topology::RepositoryTopology,
};

use super::GitCliAdapter;
use super::error::repository_redirect;

impl RepositorySnapshotPort for GitCliAdapter {
    fn collect(&self, request: SnapshotRequest<'_>) -> Result<RepositorySnapshot, CollectionError> {
        let topology = RepositoryTopology::inspect(request.repository())?;
        self.validate_object_store(request.repository(), &topology)?;
        let format = self.object_format(request.repository())?;
        let shallow = self.is_shallow(request.repository())?;
        let revision = self.resolve_revision(request.repository(), request.revision(), format)?;
        let commit_time = self.commit_time(request.repository(), &revision)?;
        let tree = self.resolve_tree(request.repository(), &revision, format)?;
        let entries = self.collect_entries(request.repository(), &tree, format)?;
        let history = self.collect_history(request.repository(), &revision, format, shallow);
        let parent_delta =
            self.collect_parent_delta(request.repository(), &revision, format, shallow);
        let status = if entries
            .iter()
            .all(|entry| entry.content().status() == EvidenceStatus::Complete)
            && history.status() == EvidenceStatus::Complete
            && parent_delta.status() == EvidenceStatus::Complete
        {
            EvidenceStatus::Complete
        } else {
            EvidenceStatus::Partial
        };
        let revision_id = RevisionId::from_str(revision.as_str()).map_err(|_| {
            CollectionError::new(
                CollectionStage::ResolveRevision,
                CollectionErrorKind::MalformedOutput,
            )
        })?;
        let tree_id = RevisionId::from_str(tree.as_str()).map_err(|_| {
            CollectionError::new(
                CollectionStage::ResolveTree,
                CollectionErrorKind::MalformedOutput,
            )
        })?;
        let source_snapshot =
            SourceSnapshot::new(request.source().clone(), revision_id, Some(tree_id));
        let final_topology = RepositoryTopology::inspect(request.repository())?;
        if final_topology != topology {
            return Err(repository_redirect());
        }
        self.validate_object_store(request.repository(), &final_topology)?;
        if self.object_format(request.repository())? != format
            || self.is_shallow(request.repository())? != shallow
        {
            return Err(repository_redirect());
        }
        Ok(RepositorySnapshot::new(
            source_snapshot,
            status,
            entries,
            history,
            parent_delta,
            GitProvenance::new(self.git_version.clone(), format),
            commit_time,
        ))
    }
}
