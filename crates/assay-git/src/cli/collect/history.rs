use std::{ffi::OsStr, path::Path};

use assay_domain::EvidenceStatus;

use crate::{
    CollectionError, CollectionStage, GitObjectFormat, GitObjectId, HistoryAvailability,
    HistoryIssue, ParentDelta, ParentDeltaIssue,
};

use super::super::error::unavailable_parent_delta;
use super::super::parse::{
    RawDiffMode, parse_first_parent, parse_lines_of_object_ids, parse_raw_diff,
};
use super::GitCliAdapter;

impl GitCliAdapter {
    pub(crate) fn collect_history(
        &self,
        repository: &Path,
        commit: &GitObjectId,
        format: GitObjectFormat,
        shallow: bool,
    ) -> HistoryAvailability {
        let maximum = self.limits.max_history_commits.saturating_add(1);
        let maximum_argument = format!("--max-count={maximum}");
        let output = match self.runner.run(
            Some(repository),
            CollectionStage::ReadHistory,
            &[
                OsStr::new("rev-list"),
                OsStr::new(&maximum_argument),
                OsStr::new(commit.as_str()),
            ],
            self.limits.max_stdout_bytes,
        ) {
            Ok(output) => output,
            Err(_) => {
                return HistoryAvailability::new(
                    EvidenceStatus::Unavailable,
                    0,
                    false,
                    Some(HistoryIssue::ProcessFailure),
                );
            }
        };
        let identifiers =
            match parse_lines_of_object_ids(&output, CollectionStage::ReadHistory, format) {
                Ok(identifiers) => identifiers,
                Err(_) => {
                    return HistoryAvailability::new(
                        EvidenceStatus::Unavailable,
                        0,
                        false,
                        Some(HistoryIssue::MalformedOutput),
                    );
                }
            };
        let depth_limited = identifiers.len() > self.limits.max_history_commits;
        let truncated = shallow || depth_limited;
        let observed = identifiers.len().min(self.limits.max_history_commits);
        HistoryAvailability::new(
            if truncated {
                EvidenceStatus::Partial
            } else {
                EvidenceStatus::Complete
            },
            observed,
            truncated,
            if shallow {
                Some(HistoryIssue::ShallowRepository)
            } else {
                depth_limited.then_some(HistoryIssue::DepthLimit)
            },
        )
    }

    pub(crate) fn collect_parent_delta(
        &self,
        repository: &Path,
        commit: &GitObjectId,
        format: GitObjectFormat,
        shallow: bool,
    ) -> ParentDelta {
        if shallow {
            return ParentDelta::new(
                EvidenceStatus::Partial,
                0,
                0,
                Some(ParentDeltaIssue::ShallowRepository),
            );
        }
        let parent_output = match self.runner.run(
            Some(repository),
            CollectionStage::ReadParentDelta,
            &[
                OsStr::new("rev-list"),
                OsStr::new("--parents"),
                OsStr::new("--max-count=1"),
                OsStr::new(commit.as_str()),
            ],
            512,
        ) {
            Ok(output) => output,
            Err(_) => return unavailable_parent_delta(ParentDeltaIssue::ProcessFailure),
        };
        let parent = match parse_first_parent(&parent_output, format) {
            Ok(None) => {
                return ParentDelta::new(EvidenceStatus::Complete, 0, 0, None);
            }
            Ok(Some(parent)) => parent,
            Err(_) => return unavailable_parent_delta(ParentDeltaIssue::MalformedOutput),
        };

        let raw = match self.diff_tree(repository, &parent, commit, false) {
            Ok(raw) => raw,
            Err(_) => return unavailable_parent_delta(ParentDeltaIssue::ProcessFailure),
        };
        let changes = match parse_raw_diff(&raw, format, RawDiffMode::NoRenames) {
            Ok(changes) => changes,
            Err(_) => return unavailable_parent_delta(ParentDeltaIssue::MalformedOutput),
        };
        if changes.len() > self.limits.max_rename_candidates {
            return ParentDelta::new(
                EvidenceStatus::Partial,
                changes.len(),
                0,
                Some(ParentDeltaIssue::RenameCandidateLimit),
            );
        }

        let renamed = match self.diff_tree(repository, &parent, commit, true) {
            Ok(raw) => raw,
            Err(_) => return unavailable_parent_delta(ParentDeltaIssue::ProcessFailure),
        };
        let renamed_changes = match parse_raw_diff(&renamed, format, RawDiffMode::FindRenames) {
            Ok(changes) => changes,
            Err(_) => return unavailable_parent_delta(ParentDeltaIssue::MalformedOutput),
        };
        ParentDelta::new(
            EvidenceStatus::Complete,
            changes.len(),
            renamed_changes
                .iter()
                .filter(|change| change.renamed)
                .count(),
            None,
        )
    }

    pub(crate) fn diff_tree(
        &self,
        repository: &Path,
        parent: &GitObjectId,
        commit: &GitObjectId,
        detect_renames: bool,
    ) -> Result<Vec<u8>, CollectionError> {
        let rename_limit = format!("-l{}", self.limits.max_rename_candidates);
        let rename_mode = if detect_renames {
            OsStr::new("--find-renames=50%")
        } else {
            OsStr::new("--no-renames")
        };
        self.runner.run(
            Some(repository),
            CollectionStage::ReadParentDelta,
            &[
                OsStr::new("diff-tree"),
                OsStr::new("--no-commit-id"),
                OsStr::new("-r"),
                OsStr::new("--raw"),
                OsStr::new("-z"),
                OsStr::new("--no-ext-diff"),
                OsStr::new("--no-textconv"),
                rename_mode,
                OsStr::new(&rename_limit),
                OsStr::new(parent.as_str()),
                OsStr::new(commit.as_str()),
            ],
            self.limits.max_stdout_bytes,
        )
    }
}
