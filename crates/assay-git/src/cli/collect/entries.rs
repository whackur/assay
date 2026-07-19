use std::{ffi::OsStr, path::Path};

use assay_domain::EvidenceStatus;

use crate::{
    CollectionError, CollectionStage, GitObjectFormat, GitObjectId, ObjectIssue, ObjectKind,
    ObjectMetadata, TrackedEntry,
};

use super::super::parse::parse_tree;
use super::GitCliAdapter;

impl GitCliAdapter {
    pub(crate) fn collect_entries(
        &self,
        repository: &Path,
        tree: &GitObjectId,
        format: GitObjectFormat,
    ) -> Result<Vec<TrackedEntry>, CollectionError> {
        let output = self.runner.run(
            Some(repository),
            CollectionStage::EnumerateTree,
            &[
                OsStr::new("ls-tree"),
                OsStr::new("-rz"),
                OsStr::new("--full-tree"),
                OsStr::new(tree.as_str()),
            ],
            self.limits.max_stdout_bytes,
        )?;
        let raw_entries = parse_tree(&output, self.limits.max_tree_entries, format)?;
        let mut entries = Vec::with_capacity(raw_entries.len());
        for raw in raw_entries {
            let content = match raw.kind {
                ObjectKind::Commit => ObjectMetadata::unresolved(
                    EvidenceStatus::Unsupported,
                    ObjectIssue::GitlinkContent,
                ),
                ObjectKind::Blob => self.collect_object_metadata(repository, &raw.object_id),
            };
            entries.push(TrackedEntry::new(
                raw.path,
                raw.mode,
                raw.kind,
                raw.object_id,
                content,
            ));
        }
        entries.sort_by(|left, right| left.path().cmp(right.path()));
        Ok(entries)
    }
}
