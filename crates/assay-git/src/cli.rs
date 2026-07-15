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
    GitObjectId, GitProvenance, HistoryAvailability, HistoryIssue, ObjectIssue, ObjectKind,
    ObjectMetadata, ParentDelta, ParentDeltaIssue, RepositoryPath, RepositorySnapshot,
    RepositorySnapshotPort, SnapshotRequest, TrackedEntry, process::GitProcessRunner,
};

const MINIMUM_GIT_MAJOR: u64 = 2;
const MINIMUM_GIT_MINOR: u64 = 47;

/// Read-only installed-Git adapter selected by ADR 0002.
pub struct GitCliAdapter {
    runner: GitProcessRunner,
    limits: CollectionLimits,
    provenance: GitProvenance,
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
            provenance: GitProvenance::new(version),
        })
    }

    fn resolve_revision(
        &self,
        repository: &Path,
        revision: &OsStr,
    ) -> Result<GitObjectId, CollectionError> {
        let mut peeled = revision.to_os_string();
        peeled.push("^{commit}");
        self.resolve_object(repository, &peeled, CollectionStage::ResolveRevision)
    }

    fn validate_object_store(&self, repository: &Path) -> Result<(), CollectionError> {
        let output = self.runner.run(
            Some(repository),
            CollectionStage::ValidateObjectStore,
            &[
                OsStr::new("rev-parse"),
                OsStr::new("--path-format=absolute"),
                OsStr::new("--git-common-dir"),
            ],
            16 * 1024,
        )?;
        let common_directory =
            path_from_git_output(single_line(&output, CollectionStage::ValidateObjectStore)?)?;
        if !common_directory.is_absolute() {
            return Err(external_object_store());
        }
        let objects = common_directory.join("objects");
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

    fn resolve_tree(
        &self,
        repository: &Path,
        revision: &GitObjectId,
    ) -> Result<GitObjectId, CollectionError> {
        let mut peeled = OsString::from(revision.as_str());
        peeled.push("^{tree}");
        self.resolve_object(repository, &peeled, CollectionStage::ResolveTree)
    }

    fn resolve_object(
        &self,
        repository: &Path,
        object: &OsStr,
        stage: CollectionStage,
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
        GitObjectId::parse(single_line(&output, stage)?, stage)
    }

    fn collect_entries(
        &self,
        repository: &Path,
        tree: &GitObjectId,
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
        let raw_entries = parse_tree(&output, self.limits.max_tree_entries)?;
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

    fn collect_history(&self, repository: &Path, commit: &GitObjectId) -> HistoryAvailability {
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
        let identifiers = match parse_lines_of_object_ids(&output, CollectionStage::ReadHistory) {
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
        let truncated = identifiers.len() > self.limits.max_history_commits;
        let observed = identifiers.len().min(self.limits.max_history_commits);
        HistoryAvailability::new(
            if truncated {
                EvidenceStatus::Partial
            } else {
                EvidenceStatus::Complete
            },
            observed,
            truncated,
            truncated.then_some(HistoryIssue::DepthLimit),
        )
    }

    fn collect_parent_delta(&self, repository: &Path, commit: &GitObjectId) -> ParentDelta {
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
        let parent = match parse_first_parent(&parent_output) {
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
        let changes = match parse_raw_diff(&raw) {
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
        let renamed_changes = match parse_raw_diff(&renamed) {
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
        self.validate_object_store(request.repository())?;
        let revision = self.resolve_revision(request.repository(), request.revision())?;
        let tree = self.resolve_tree(request.repository(), &revision)?;
        let entries = self.collect_entries(request.repository(), &tree)?;
        let history = self.collect_history(request.repository(), &revision);
        let parent_delta = self.collect_parent_delta(request.repository(), &revision);
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
        Ok(RepositorySnapshot::new(
            source_snapshot,
            status,
            entries,
            history,
            parent_delta,
            self.provenance.clone(),
        ))
    }
}

impl std::fmt::Debug for GitCliAdapter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GitCliAdapter")
            .field("executable", &"<trusted-executable>")
            .field("limits", &self.limits)
            .field("provenance", &self.provenance)
            .finish()
    }
}

struct RawTreeEntry {
    path: RepositoryPath,
    mode: EntryMode,
    kind: ObjectKind,
    object_id: GitObjectId,
}

fn parse_tree(output: &[u8], maximum: usize) -> Result<Vec<RawTreeEntry>, CollectionError> {
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
            object_id: GitObjectId::parse(fields[2].as_bytes(), CollectionStage::EnumerateTree)?,
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
    if version.is_empty()
        || version.len() > 80
        || !version
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    {
        return Err(incompatible_git());
    }
    let mut parts = version.split('.');
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
) -> Result<Vec<GitObjectId>, CollectionError> {
    if output.is_empty() || !output.ends_with(b"\n") {
        return Err(CollectionError::new(
            stage,
            CollectionErrorKind::MalformedOutput,
        ));
    }
    output[..output.len() - 1]
        .split(|byte| *byte == b'\n')
        .map(|line| GitObjectId::parse(line, stage))
        .collect()
}

fn parse_first_parent(output: &[u8]) -> Result<Option<GitObjectId>, CollectionError> {
    let line = single_line(output, CollectionStage::ReadParentDelta)?;
    let fields = line.split(|byte| *byte == b' ').collect::<Vec<_>>();
    if fields.is_empty() {
        return Err(CollectionError::new(
            CollectionStage::ReadParentDelta,
            CollectionErrorKind::MalformedOutput,
        ));
    }
    for field in &fields {
        GitObjectId::parse(field, CollectionStage::ReadParentDelta)?;
    }
    if fields.len() == 1 {
        Ok(None)
    } else {
        GitObjectId::parse(fields[1], CollectionStage::ReadParentDelta).map(Some)
    }
}

struct RawChange {
    renamed: bool,
}

fn parse_raw_diff(output: &[u8]) -> Result<Vec<RawChange>, CollectionError> {
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
            || !valid_diff_mode(fields[0].strip_prefix(b":").unwrap_or_default())
            || !valid_diff_mode(fields[1])
            || !valid_diff_object_id(fields[2])
            || !valid_diff_object_id(fields[3])
            || fields[2].len() != fields[3].len()
            || !valid_diff_status(fields[4])
        {
            return Err(malformed_parent_delta());
        }
        let status = fields[4][0];
        if index >= segments.len() || segments[index].is_empty() {
            return Err(CollectionError::new(
                CollectionStage::ReadParentDelta,
                CollectionErrorKind::MalformedOutput,
            ));
        }
        index += 1;
        let renamed = matches!(status, b'R' | b'C');
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

fn valid_diff_mode(value: &[u8]) -> bool {
    matches!(
        value,
        b"000000" | b"100644" | b"100755" | b"120000" | b"160000"
    )
}

fn valid_diff_object_id(value: &[u8]) -> bool {
    matches!(value.len(), 40 | 64)
        && value
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn valid_diff_status(value: &[u8]) -> bool {
    match value {
        [b'A' | b'D' | b'M' | b'T'] => true,
        [b'C' | b'R', hundreds, tens, ones] => {
            hundreds.is_ascii_digit()
                && tens.is_ascii_digit()
                && ones.is_ascii_digit()
                && (hundreds, tens, ones) <= (&b'1', &b'0', &b'0')
        }
        _ => false,
    }
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
