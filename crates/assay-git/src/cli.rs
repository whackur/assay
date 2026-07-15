use std::{
    ffi::{OsStr, OsString},
    fs,
    path::{Path, PathBuf},
    str::FromStr,
};

use assay_domain::{ContentHash, EvidenceStatus, RevisionId, SourceSnapshot};
use sha2::{Digest, Sha256};

use crate::{
    CollectionError, CollectionErrorKind, CollectionLimits, CollectionStage, EntryMode,
    GitObjectFormat, GitObjectId, GitProvenance, HistoryAvailability, HistoryIssue, ObjectIssue,
    ObjectKind, ObjectMetadata, ParentDelta, ParentDeltaIssue, RepositoryPath, RepositorySnapshot,
    RepositorySnapshotPort, SnapshotRequest, TrackedEntry,
    process::GitProcessRunner,
    topology::{RepositoryKind, RepositoryTopology},
};

const MINIMUM_GIT_MAJOR: u64 = 2;
const MINIMUM_GIT_MINOR: u64 = 47;

/// Read-only installed-Git adapter selected by ADR 0002.
pub struct GitCliAdapter {
    runner: GitProcessRunner,
    limits: CollectionLimits,
    git_version: String,
}

impl GitCliAdapter {
    /// Probes one deployment-trusted absolute executable exactly once.
    ///
    /// The probe requires Git 2.47 or newer and the global
    /// `--no-lazy-fetch` option. Repository content never selects this path.
    pub fn from_trusted_executable(
        executable: PathBuf,
        limits: CollectionLimits,
    ) -> Result<Self, CollectionError> {
        if !limits.is_valid() {
            return Err(CollectionError::new(
                CollectionStage::ConfigureAdapter,
                CollectionErrorKind::InvalidLimits,
            ));
        }
        if !executable.is_absolute() {
            return Err(CollectionError::new(
                CollectionStage::ConfigureAdapter,
                CollectionErrorKind::UntrustedExecutable,
            ));
        }
        let runner = GitProcessRunner::new(executable, limits);
        let output = runner
            .run(
                None,
                CollectionStage::ProbeCapabilities,
                &[OsStr::new("version")],
                256,
            )
            .map_err(|error| {
                if error.kind() == CollectionErrorKind::NonZeroExit {
                    incompatible_git()
                } else {
                    error
                }
            })?;
        let version = parse_version(&output)?;
        Ok(Self {
            runner,
            limits,
            git_version: version,
        })
    }

    fn resolve_revision(
        &self,
        repository: &Path,
        revision: &OsStr,
        format: GitObjectFormat,
    ) -> Result<GitObjectId, CollectionError> {
        let mut peeled = revision.to_os_string();
        peeled.push("^{commit}");
        self.resolve_object(
            repository,
            &peeled,
            CollectionStage::ResolveRevision,
            format,
        )
    }

    fn validate_object_store(
        &self,
        repository: &Path,
        topology: &RepositoryTopology,
    ) -> Result<(), CollectionError> {
        let git_directory = self.reported_path(
            repository,
            &[
                OsStr::new("rev-parse"),
                OsStr::new("--path-format=absolute"),
                OsStr::new("--absolute-git-dir"),
            ],
        )?;
        let common_directory = self.reported_path(
            repository,
            &[
                OsStr::new("rev-parse"),
                OsStr::new("--path-format=absolute"),
                OsStr::new("--git-common-dir"),
            ],
        )?;
        if git_directory != topology.git_directory()
            || common_directory != topology.common_directory()
        {
            return Err(repository_redirect());
        }
        let bare_output = self.runner.run(
            Some(repository),
            CollectionStage::ValidateObjectStore,
            &[OsStr::new("rev-parse"), OsStr::new("--is-bare-repository")],
            16,
        )?;
        let reported_bare = parse_boolean(&bare_output, CollectionStage::ValidateObjectStore)?;
        if reported_bare != (topology.kind() == RepositoryKind::Bare) {
            return Err(repository_redirect());
        }
        if topology.kind() != RepositoryKind::Bare {
            let top_level = self.reported_path(
                repository,
                &[
                    OsStr::new("rev-parse"),
                    OsStr::new("--path-format=absolute"),
                    OsStr::new("--show-toplevel"),
                ],
            )?;
            if top_level != topology.repository() {
                return Err(repository_redirect());
            }
        }

        let objects = topology.common_directory().join("objects");
        let metadata = fs::symlink_metadata(&objects).map_err(|_| {
            CollectionError::new(
                CollectionStage::ValidateObjectStore,
                CollectionErrorKind::ExternalObjectStore,
            )
        })?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(external_object_store());
        }
        for alternates in [
            objects.join("info/alternates"),
            objects.join("info/http-alternates"),
        ] {
            match fs::symlink_metadata(alternates) {
                Ok(_) => return Err(external_object_store()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => {
                    return Err(CollectionError::new(
                        CollectionStage::ValidateObjectStore,
                        CollectionErrorKind::Io,
                    ));
                }
            }
        }
        reject_object_store_symlinks(&objects, self.limits.max_object_store_entries)
    }

    fn reported_path(
        &self,
        repository: &Path,
        arguments: &[&OsStr],
    ) -> Result<PathBuf, CollectionError> {
        let output = self.runner.run(
            Some(repository),
            CollectionStage::ValidateObjectStore,
            arguments,
            16 * 1024,
        )?;
        let path =
            path_from_git_output(single_line(&output, CollectionStage::ValidateObjectStore)?)?;
        if !path.is_absolute() {
            return Err(repository_redirect());
        }
        fs::canonicalize(path).map_err(|_| repository_redirect())
    }

    fn object_format(&self, repository: &Path) -> Result<GitObjectFormat, CollectionError> {
        let output = self.runner.run(
            Some(repository),
            CollectionStage::ValidateObjectStore,
            &[
                OsStr::new("rev-parse"),
                OsStr::new("--show-object-format=storage"),
            ],
            32,
        )?;
        match single_line(&output, CollectionStage::ValidateObjectStore)? {
            b"sha1" => Ok(GitObjectFormat::Sha1),
            b"sha256" => Ok(GitObjectFormat::Sha256),
            _ => Err(CollectionError::new(
                CollectionStage::ValidateObjectStore,
                CollectionErrorKind::MalformedOutput,
            )),
        }
    }

    fn is_shallow(&self, repository: &Path) -> Result<bool, CollectionError> {
        let output = self.runner.run(
            Some(repository),
            CollectionStage::ReadHistory,
            &[
                OsStr::new("rev-parse"),
                OsStr::new("--is-shallow-repository"),
            ],
            16,
        )?;
        parse_boolean(&output, CollectionStage::ReadHistory)
    }

    fn resolve_tree(
        &self,
        repository: &Path,
        revision: &GitObjectId,
        format: GitObjectFormat,
    ) -> Result<GitObjectId, CollectionError> {
        let mut peeled = OsString::from(revision.as_str());
        peeled.push("^{tree}");
        self.resolve_object(repository, &peeled, CollectionStage::ResolveTree, format)
    }

    fn resolve_object(
        &self,
        repository: &Path,
        object: &OsStr,
        stage: CollectionStage,
        format: GitObjectFormat,
    ) -> Result<GitObjectId, CollectionError> {
        let output = self.runner.run(
            Some(repository),
            stage,
            &[
                OsStr::new("rev-parse"),
                OsStr::new("--verify"),
                OsStr::new("--end-of-options"),
                object,
            ],
            128,
        )?;
        GitObjectId::parse(single_line(&output, stage)?, stage, format)
    }

    fn collect_entries(
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

    fn collect_object_metadata(
        &self,
        repository: &Path,
        object_id: &GitObjectId,
    ) -> ObjectMetadata {
        let size_output = match self.runner.run(
            Some(repository),
            CollectionStage::ReadObjectMetadata,
            &[
                OsStr::new("cat-file"),
                OsStr::new("-s"),
                OsStr::new(object_id.as_str()),
            ],
            64,
        ) {
            Ok(output) => output,
            Err(error) => {
                return ObjectMetadata::unresolved(
                    EvidenceStatus::Unavailable,
                    object_issue(error.kind()),
                );
            }
        };
        let size = match parse_decimal(&size_output, CollectionStage::ReadObjectMetadata) {
            Ok(size) => size,
            Err(_) => {
                return ObjectMetadata::unresolved(
                    EvidenceStatus::Unavailable,
                    ObjectIssue::MalformedMetadata,
                );
            }
        };
        if size > self.limits.max_object_bytes {
            return ObjectMetadata::limited(size);
        }
        let stdout_limit = match usize::try_from(self.limits.max_object_bytes) {
            Ok(limit) => limit,
            Err(_) => self.limits.max_stdout_bytes,
        };
        let bytes = match self.runner.run(
            Some(repository),
            CollectionStage::HashObject,
            &[
                OsStr::new("cat-file"),
                OsStr::new("blob"),
                OsStr::new(object_id.as_str()),
            ],
            stdout_limit,
        ) {
            Ok(bytes) => bytes,
            Err(error) => {
                return ObjectMetadata::unresolved(
                    EvidenceStatus::Unavailable,
                    object_issue(error.kind()),
                );
            }
        };
        if u64::try_from(bytes.len()).ok() != Some(size) {
            return ObjectMetadata::unresolved(
                EvidenceStatus::Unavailable,
                ObjectIssue::MalformedMetadata,
            );
        }
        let digest = Sha256::digest(&bytes);
        let mut encoded = String::with_capacity(71);
        encoded.push_str("sha256:");
        for byte in digest {
            use std::fmt::Write;
            write!(&mut encoded, "{byte:02x}").expect("writing to a String cannot fail");
        }
        let content_hash = ContentHash::from_str(&encoded)
            .expect("SHA-256 output always satisfies the domain digest invariant");
        ObjectMetadata::complete(size, content_hash)
    }

    fn collect_history(
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

    fn collect_parent_delta(
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

    fn diff_tree(
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

impl RepositorySnapshotPort for GitCliAdapter {
    fn collect(&self, request: SnapshotRequest<'_>) -> Result<RepositorySnapshot, CollectionError> {
        let topology = RepositoryTopology::inspect(request.repository())?;
        self.validate_object_store(request.repository(), &topology)?;
        let format = self.object_format(request.repository())?;
        let shallow = self.is_shallow(request.repository())?;
        let revision = self.resolve_revision(request.repository(), request.revision(), format)?;
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
        ))
    }
}

impl std::fmt::Debug for GitCliAdapter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GitCliAdapter")
            .field("executable", &"<trusted-executable>")
            .field("limits", &self.limits)
            .field("git_version", &self.git_version)
            .finish()
    }
}

struct RawTreeEntry {
    path: RepositoryPath,
    mode: EntryMode,
    kind: ObjectKind,
    object_id: GitObjectId,
}

fn parse_tree(
    output: &[u8],
    maximum: usize,
    format: GitObjectFormat,
) -> Result<Vec<RawTreeEntry>, CollectionError> {
    if output.is_empty() {
        return Ok(Vec::new());
    }
    if !output.ends_with(&[0]) {
        return Err(CollectionError::new(
            CollectionStage::EnumerateTree,
            CollectionErrorKind::MalformedOutput,
        ));
    }
    let mut entries = Vec::new();
    let records = &output[..output.len() - 1];
    for record in records.split(|byte| *byte == 0) {
        if entries.len() == maximum {
            return Err(CollectionError::new(
                CollectionStage::EnumerateTree,
                CollectionErrorKind::RecordLimit,
            ));
        }
        let tab = record
            .iter()
            .position(|byte| *byte == b'\t')
            .ok_or_else(|| {
                CollectionError::new(
                    CollectionStage::EnumerateTree,
                    CollectionErrorKind::MalformedOutput,
                )
            })?;
        let header = std::str::from_utf8(&record[..tab]).map_err(|_| {
            CollectionError::new(
                CollectionStage::EnumerateTree,
                CollectionErrorKind::MalformedOutput,
            )
        })?;
        let fields = header.split_ascii_whitespace().collect::<Vec<_>>();
        if fields.len() != 3 {
            return Err(CollectionError::new(
                CollectionStage::EnumerateTree,
                CollectionErrorKind::MalformedOutput,
            ));
        }
        let (mode, expected_kind) = match fields[0] {
            "100644" => (EntryMode::Regular, ObjectKind::Blob),
            "100755" => (EntryMode::Executable, ObjectKind::Blob),
            "120000" => (EntryMode::SymbolicLink, ObjectKind::Blob),
            "160000" => (EntryMode::Gitlink, ObjectKind::Commit),
            _ => {
                return Err(CollectionError::new(
                    CollectionStage::EnumerateTree,
                    CollectionErrorKind::MalformedOutput,
                ));
            }
        };
        let kind = match fields[1] {
            "blob" => ObjectKind::Blob,
            "commit" => ObjectKind::Commit,
            _ => {
                return Err(CollectionError::new(
                    CollectionStage::EnumerateTree,
                    CollectionErrorKind::MalformedOutput,
                ));
            }
        };
        if kind != expected_kind {
            return Err(CollectionError::new(
                CollectionStage::EnumerateTree,
                CollectionErrorKind::MalformedOutput,
            ));
        }
        entries.push(RawTreeEntry {
            path: RepositoryPath::new(record[tab + 1..].to_vec())?,
            mode,
            kind,
            object_id: GitObjectId::parse(
                fields[2].as_bytes(),
                CollectionStage::EnumerateTree,
                format,
            )?,
        });
    }
    if entries.windows(2).any(|pair| pair[0].path >= pair[1].path) {
        return Err(CollectionError::new(
            CollectionStage::EnumerateTree,
            CollectionErrorKind::MalformedOutput,
        ));
    }
    Ok(entries)
}

fn parse_version(output: &[u8]) -> Result<String, CollectionError> {
    let line = std::str::from_utf8(single_line(output, CollectionStage::ProbeCapabilities)?)
        .map_err(|_| incompatible_git())?;
    let version = line
        .strip_prefix("git version ")
        .ok_or_else(incompatible_git)?;
    if version.is_empty() || version.len() > 80 || version.trim() != version {
        return Err(incompatible_git());
    }
    if !version.bytes().all(|byte| {
        byte.is_ascii_alphanumeric()
            || matches!(byte, b' ' | b'.' | b'-' | b'_' | b'(' | b')' | b'+')
    }) {
        return Err(incompatible_git());
    }
    let numeric_end = version
        .bytes()
        .take_while(|byte| byte.is_ascii_digit() || *byte == b'.')
        .count();
    let numeric = version[..numeric_end].trim_end_matches('.');
    let suffix = &version[numeric.len()..];
    if numeric
        .split('.')
        .any(|part| part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return Err(incompatible_git());
    }
    let dotted_suffix = suffix.strip_prefix('.').is_some_and(|value| {
        !value.is_empty()
            && value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'+')
            })
    });
    let parenthesized_suffix = suffix
        .strip_prefix(" (")
        .and_then(|value| value.strip_suffix(')'))
        .is_some_and(|value| {
            !value.is_empty()
                && value.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b' ' | b'.' | b'-' | b'_' | b'+')
                })
        });
    if !suffix.is_empty() && !dotted_suffix && !parenthesized_suffix {
        return Err(incompatible_git());
    }
    let mut parts = numeric.split('.');
    let major = parts
        .next()
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or_else(incompatible_git)?;
    let minor = parts
        .next()
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or_else(incompatible_git)?;
    if major < MINIMUM_GIT_MAJOR || (major == MINIMUM_GIT_MAJOR && minor < MINIMUM_GIT_MINOR) {
        return Err(incompatible_git());
    }
    Ok(version.to_owned())
}

fn incompatible_git() -> CollectionError {
    CollectionError::new(
        CollectionStage::ProbeCapabilities,
        CollectionErrorKind::IncompatibleGit,
    )
}

fn single_line(output: &[u8], stage: CollectionStage) -> Result<&[u8], CollectionError> {
    let value = output.strip_suffix(b"\n").unwrap_or(output);
    let value = value.strip_suffix(b"\r").unwrap_or(value);
    if value.is_empty() || value.contains(&b'\n') || value.contains(&b'\r') {
        return Err(CollectionError::new(
            stage,
            CollectionErrorKind::MalformedOutput,
        ));
    }
    Ok(value)
}

fn parse_boolean(output: &[u8], stage: CollectionStage) -> Result<bool, CollectionError> {
    match single_line(output, stage)? {
        b"true" => Ok(true),
        b"false" => Ok(false),
        _ => Err(CollectionError::new(
            stage,
            CollectionErrorKind::MalformedOutput,
        )),
    }
}

fn parse_decimal(output: &[u8], stage: CollectionStage) -> Result<u64, CollectionError> {
    let value = std::str::from_utf8(single_line(output, stage)?)
        .map_err(|_| CollectionError::new(stage, CollectionErrorKind::MalformedOutput))?;
    if value.starts_with('+') || (value.starts_with('0') && value.len() > 1) {
        return Err(CollectionError::new(
            stage,
            CollectionErrorKind::MalformedOutput,
        ));
    }
    value
        .parse::<u64>()
        .map_err(|_| CollectionError::new(stage, CollectionErrorKind::MalformedOutput))
}

fn parse_lines_of_object_ids(
    output: &[u8],
    stage: CollectionStage,
    format: GitObjectFormat,
) -> Result<Vec<GitObjectId>, CollectionError> {
    if output.is_empty() || !output.ends_with(b"\n") {
        return Err(CollectionError::new(
            stage,
            CollectionErrorKind::MalformedOutput,
        ));
    }
    output[..output.len() - 1]
        .split(|byte| *byte == b'\n')
        .map(|line| GitObjectId::parse(line, stage, format))
        .collect()
}

fn parse_first_parent(
    output: &[u8],
    format: GitObjectFormat,
) -> Result<Option<GitObjectId>, CollectionError> {
    let line = single_line(output, CollectionStage::ReadParentDelta)?;
    let fields = line.split(|byte| *byte == b' ').collect::<Vec<_>>();
    if fields.is_empty() {
        return Err(CollectionError::new(
            CollectionStage::ReadParentDelta,
            CollectionErrorKind::MalformedOutput,
        ));
    }
    for field in &fields {
        GitObjectId::parse(field, CollectionStage::ReadParentDelta, format)?;
    }
    if fields.len() == 1 {
        Ok(None)
    } else {
        GitObjectId::parse(fields[1], CollectionStage::ReadParentDelta, format).map(Some)
    }
}

struct RawChange {
    renamed: bool,
}

#[derive(Clone, Copy)]
enum RawDiffMode {
    NoRenames,
    FindRenames,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum DiffModeClass {
    Absent,
    RegularBlob,
    Symlink,
    Gitlink,
}

fn parse_raw_diff(
    output: &[u8],
    format: GitObjectFormat,
    mode: RawDiffMode,
) -> Result<Vec<RawChange>, CollectionError> {
    if output.is_empty() {
        return Ok(Vec::new());
    }
    if !output.ends_with(&[0]) {
        return Err(CollectionError::new(
            CollectionStage::ReadParentDelta,
            CollectionErrorKind::MalformedOutput,
        ));
    }
    let segments = output[..output.len() - 1]
        .split(|byte| *byte == 0)
        .collect::<Vec<_>>();
    let mut index = 0;
    let mut changes = Vec::new();
    while index < segments.len() {
        let header = segments[index];
        index += 1;
        let fields = header.split(|byte| *byte == b' ').collect::<Vec<_>>();
        if fields.len() != 5
            || !valid_diff_object_id(fields[2], format)
            || !valid_diff_object_id(fields[3], format)
        {
            return Err(malformed_parent_delta());
        }
        let old_mode = parse_diff_mode(fields[0].strip_prefix(b":").unwrap_or_default())
            .ok_or_else(malformed_parent_delta)?;
        let new_mode = parse_diff_mode(fields[1]).ok_or_else(malformed_parent_delta)?;
        let status = parse_diff_status(&fields, old_mode, new_mode, mode)?;
        if index >= segments.len() || segments[index].is_empty() {
            return Err(CollectionError::new(
                CollectionStage::ReadParentDelta,
                CollectionErrorKind::MalformedOutput,
            ));
        }
        index += 1;
        let renamed = status == b'R';
        if renamed {
            if index >= segments.len() || segments[index].is_empty() {
                return Err(CollectionError::new(
                    CollectionStage::ReadParentDelta,
                    CollectionErrorKind::MalformedOutput,
                ));
            }
            index += 1;
        }
        changes.push(RawChange { renamed });
    }
    Ok(changes)
}

fn parse_diff_mode(value: &[u8]) -> Option<DiffModeClass> {
    match value {
        b"000000" => Some(DiffModeClass::Absent),
        b"100644" | b"100755" => Some(DiffModeClass::RegularBlob),
        b"120000" => Some(DiffModeClass::Symlink),
        b"160000" => Some(DiffModeClass::Gitlink),
        _ => None,
    }
}

fn valid_diff_object_id(value: &[u8], format: GitObjectFormat) -> bool {
    value.len() == format.identifier_length()
        && value
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn parse_diff_status(
    fields: &[&[u8]],
    old_mode: DiffModeClass,
    new_mode: DiffModeClass,
    mode: RawDiffMode,
) -> Result<u8, CollectionError> {
    let old_null = fields[2].iter().all(|byte| *byte == b'0');
    let new_null = fields[3].iter().all(|byte| *byte == b'0');
    let old_absent = old_mode == DiffModeClass::Absent;
    let new_absent = new_mode == DiffModeClass::Absent;
    let status = fields[4];
    let valid = match status {
        [b'A'] => old_absent && old_null && !new_absent && !new_null,
        [b'D'] => !old_absent && !old_null && new_absent && new_null,
        [b'M'] => !old_absent && !old_null && !new_absent && !new_null && old_mode == new_mode,
        [b'T'] => !old_absent && !old_null && !new_absent && !new_null && old_mode != new_mode,
        [b'R', hundreds, tens, ones] if matches!(mode, RawDiffMode::FindRenames) => {
            !old_absent
                && !old_null
                && !new_absent
                && !new_null
                && old_mode == new_mode
                && valid_rename_score(*hundreds, *tens, *ones)
        }
        _ => false,
    };
    if valid {
        Ok(status[0])
    } else {
        Err(malformed_parent_delta())
    }
}

fn valid_rename_score(hundreds: u8, tens: u8, ones: u8) -> bool {
    let valid_digits = hundreds.is_ascii_digit() && tens.is_ascii_digit() && ones.is_ascii_digit();
    if !valid_digits {
        return false;
    }
    let score = usize::from(hundreds - b'0') * 100
        + usize::from(tens - b'0') * 10
        + usize::from(ones - b'0');
    (50..=100).contains(&score)
}

fn malformed_parent_delta() -> CollectionError {
    CollectionError::new(
        CollectionStage::ReadParentDelta,
        CollectionErrorKind::MalformedOutput,
    )
}

fn external_object_store() -> CollectionError {
    CollectionError::new(
        CollectionStage::ValidateObjectStore,
        CollectionErrorKind::ExternalObjectStore,
    )
}

fn repository_redirect() -> CollectionError {
    CollectionError::new(
        CollectionStage::ValidateObjectStore,
        CollectionErrorKind::RepositoryRedirect,
    )
}

fn reject_object_store_symlinks(
    objects: &Path,
    maximum_entries: usize,
) -> Result<(), CollectionError> {
    let mut pending = vec![objects.to_path_buf()];
    let mut inspected = 0_usize;
    while let Some(directory) = pending.pop() {
        let children = fs::read_dir(directory).map_err(|_| {
            CollectionError::new(
                CollectionStage::ValidateObjectStore,
                CollectionErrorKind::Io,
            )
        })?;
        for child in children {
            let child = child.map_err(|_| {
                CollectionError::new(
                    CollectionStage::ValidateObjectStore,
                    CollectionErrorKind::Io,
                )
            })?;
            inspected = inspected.checked_add(1).ok_or_else(|| {
                CollectionError::new(
                    CollectionStage::ValidateObjectStore,
                    CollectionErrorKind::RecordLimit,
                )
            })?;
            if inspected > maximum_entries {
                return Err(CollectionError::new(
                    CollectionStage::ValidateObjectStore,
                    CollectionErrorKind::RecordLimit,
                ));
            }
            let metadata = fs::symlink_metadata(child.path()).map_err(|_| {
                CollectionError::new(
                    CollectionStage::ValidateObjectStore,
                    CollectionErrorKind::Io,
                )
            })?;
            if metadata.file_type().is_symlink() {
                return Err(external_object_store());
            }
            if metadata.is_dir() {
                pending.push(child.path());
            }
        }
    }
    Ok(())
}

#[cfg(unix)]
fn path_from_git_output(bytes: &[u8]) -> Result<PathBuf, CollectionError> {
    use std::os::unix::ffi::OsStringExt;

    Ok(PathBuf::from(OsString::from_vec(bytes.to_vec())))
}

#[cfg(not(unix))]
fn path_from_git_output(bytes: &[u8]) -> Result<PathBuf, CollectionError> {
    let value = std::str::from_utf8(bytes).map_err(|_| {
        CollectionError::new(
            CollectionStage::ValidateObjectStore,
            CollectionErrorKind::MalformedOutput,
        )
    })?;
    Ok(PathBuf::from(value))
}

fn object_issue(kind: CollectionErrorKind) -> ObjectIssue {
    match kind {
        CollectionErrorKind::Timeout => ObjectIssue::Timeout,
        CollectionErrorKind::OutputLimit => ObjectIssue::OutputLimit,
        CollectionErrorKind::MalformedOutput => ObjectIssue::MalformedMetadata,
        _ => ObjectIssue::MissingOrUnreadable,
    }
}

fn unavailable_parent_delta(issue: ParentDeltaIssue) -> ParentDelta {
    ParentDelta::new(EvidenceStatus::Unavailable, 0, 0, Some(issue))
}
