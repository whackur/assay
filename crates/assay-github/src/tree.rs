use std::{collections::BTreeSet, error::Error, fmt, str::FromStr};

use assay_domain::RuleSetHash;
use serde::{Deserialize, de, de::DeserializeSeed as _};

use crate::{
    BlobAnalysisKey, BlobCacheLookup, BlobCacheState, CacheVersion, CollectionError,
    CollectionErrorKind, CollectionStage, GitHubObjectId, GitHubResponse, RateLimitState,
    collection::{LimitedReader, content_length_exceeds, is_response_limit},
};

const DEFAULT_MAX_ENTRIES: usize = 250_000;
const DEFAULT_MAX_RESPONSE_BYTES: usize = 128 * 1024 * 1024;
const DEFAULT_MAX_PATH_BYTES: usize = 4_096;
const DEFAULT_MAX_BOUNDARIES: usize = 4_096;

/// Hard bounds for one recursive GitHub tree response.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TreeCollectionLimits {
    max_entries: usize,
    max_response_bytes: usize,
    max_path_bytes: usize,
    max_boundaries: usize,
}

impl TreeCollectionLimits {
    /// Creates positive hard limits for tree parsing and retained metadata.
    pub fn new(
        max_entries: usize,
        max_response_bytes: usize,
        max_path_bytes: usize,
        max_boundaries: usize,
    ) -> Result<Self, &'static str> {
        if [
            max_entries,
            max_response_bytes,
            max_path_bytes,
            max_boundaries,
        ]
        .contains(&0)
        {
            return Err("tree collection limits must be positive");
        }
        Ok(Self {
            max_entries,
            max_response_bytes,
            max_path_bytes,
            max_boundaries,
        })
    }

    /// Returns the maximum number of entries processed in detail.
    pub const fn max_entries(self) -> usize {
        self.max_entries
    }

    /// Returns the maximum response bytes read.
    pub const fn max_response_bytes(self) -> usize {
        self.max_response_bytes
    }

    /// Returns the maximum bytes in one retained path.
    pub const fn max_path_bytes(self) -> usize {
        self.max_path_bytes
    }

    /// Returns the maximum retained project boundaries.
    pub const fn max_boundaries(self) -> usize {
        self.max_boundaries
    }
}

impl Default for TreeCollectionLimits {
    fn default() -> Self {
        Self {
            max_entries: DEFAULT_MAX_ENTRIES,
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
            max_path_bytes: DEFAULT_MAX_PATH_BYTES,
            max_boundaries: DEFAULT_MAX_BOUNDARIES,
        }
    }
}

/// Overall availability of bounded tree collection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CollectionStatus {
    /// Every provider entry was processed within local bounds.
    Complete,
    /// Some entries or boundary facts were unavailable due to an explicit bound.
    Partial,
}

/// A reason why a successful tree collection is partial.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TreePartialReason {
    /// GitHub reported that its recursive tree response was truncated.
    ProviderTruncated,
    /// More entries existed than the configured detail bound.
    EntryLimit,
    /// At least one path exceeded the configured portable path bound.
    PathLimit,
    /// More project roots were detected than the retained boundary bound.
    BoundaryLimit,
}

/// One uncached or cache-unavailable blob sent to a downstream analyzer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlobWorkItem {
    path: String,
    blob: GitHubObjectId,
    size_bytes: Option<u64>,
    cache_state: BlobCacheState,
}

impl BlobWorkItem {
    /// Returns the repository-relative UTF-8 path.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the immutable blob object identifier.
    pub const fn blob(&self) -> &GitHubObjectId {
        &self.blob
    }

    /// Returns GitHub's reported blob size when available.
    pub const fn size_bytes(&self) -> Option<u64> {
        self.size_bytes
    }

    /// Returns whether the cache missed or was unavailable.
    pub const fn cache_state(&self) -> BlobCacheState {
        self.cache_state
    }
}

/// A downstream streaming failure with no path or source content.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TreeSinkError {
    code: &'static str,
}

impl TreeSinkError {
    /// Creates an error from a stable snake-case code.
    pub fn new(code: &'static str) -> Result<Self, &'static str> {
        if code.is_empty()
            || code.len() > 64
            || !code
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        {
            return Err("invalid tree sink error code");
        }
        Ok(Self { code })
    }

    /// Returns the stable error code.
    pub const fn code(&self) -> &'static str {
        self.code
    }
}

impl fmt::Display for TreeSinkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "tree sink failed: {}", self.code)
    }
}

impl Error for TreeSinkError {}

/// Streaming consumer for blob analyses that cannot be reused from cache.
pub trait TreeSink {
    /// Accepts one bounded repository-relative blob work item.
    fn accept(&mut self, item: BlobWorkItem) -> Result<(), TreeSinkError>;
}

/// Bounded collection facts. Counts are observations, not quality scores.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TreeCollectionSummary {
    status: CollectionStatus,
    observed_entries: usize,
    observed_blobs: usize,
    cache_hits: usize,
    cache_misses: usize,
    cache_unavailable: usize,
    project_boundaries: Vec<String>,
    partial_reasons: Vec<TreePartialReason>,
    rate_limit: RateLimitState,
}

impl TreeCollectionSummary {
    /// Returns complete or partial availability.
    pub const fn status(&self) -> CollectionStatus {
        self.status
    }

    /// Returns every entry observed while streaming the response.
    pub const fn observed_entries(&self) -> usize {
        self.observed_entries
    }

    /// Returns blobs processed in detail within the local entry limit.
    pub const fn observed_blobs(&self) -> usize {
        self.observed_blobs
    }

    /// Returns blob-analysis cache hits.
    pub const fn cache_hits(&self) -> usize {
        self.cache_hits
    }

    /// Returns blob-analysis cache misses.
    pub const fn cache_misses(&self) -> usize {
        self.cache_misses
    }

    /// Returns lookups whose cache state was unavailable.
    pub const fn cache_unavailable(&self) -> usize {
        self.cache_unavailable
    }

    /// Returns sorted repository-relative project roots. `.` is repository root.
    pub fn project_boundaries(&self) -> &[String] {
        &self.project_boundaries
    }

    /// Returns explicit reasons for partial tree evidence.
    pub fn partial_reasons(&self) -> &[TreePartialReason] {
        &self.partial_reasons
    }

    /// Returns API budget state from the tree response.
    pub const fn rate_limit(&self) -> &RateLimitState {
        &self.rate_limit
    }
}

pub(crate) fn deserialize_tree_response<C: BlobCacheLookup, S: TreeSink>(
    response: GitHubResponse,
    rate_limit: RateLimitState,
    cache: &C,
    analyzer_version: CacheVersion,
    rule_set_hash: RuleSetHash,
    limits: TreeCollectionLimits,
    sink: &mut S,
) -> Result<TreeCollectionSummary, CollectionError> {
    if content_length_exceeds(&response, limits.max_response_bytes()) {
        return Err(CollectionError::new(
            CollectionErrorKind::ResponseLimit,
            CollectionStage::Tree,
        ));
    }
    let reader = LimitedReader::new(response.into_body(), limits.max_response_bytes());
    let mut handler = TreeHandler {
        cache,
        analyzer_version,
        rule_set_hash,
        limits,
        sink,
        observed_entries: 0,
        observed_blobs: 0,
        cache_hits: 0,
        cache_misses: 0,
        cache_unavailable: 0,
        boundaries: BTreeSet::new(),
        partial_reasons: BTreeSet::new(),
    };
    let mut deserializer = serde_json::Deserializer::from_reader(reader);
    let envelope = TreeEnvelopeSeed {
        handler: &mut handler,
    }
    .deserialize(&mut deserializer)
    .map_err(map_tree_parse_error)?;
    deserializer.end().map_err(map_tree_parse_error)?;

    if envelope.truncated {
        handler
            .partial_reasons
            .insert(TreePartialReason::ProviderTruncated);
    }
    let partial_reasons: Vec<_> = handler.partial_reasons.into_iter().collect();
    let status = if partial_reasons.is_empty() {
        CollectionStatus::Complete
    } else {
        CollectionStatus::Partial
    };
    Ok(TreeCollectionSummary {
        status,
        observed_entries: handler.observed_entries,
        observed_blobs: handler.observed_blobs,
        cache_hits: handler.cache_hits,
        cache_misses: handler.cache_misses,
        cache_unavailable: handler.cache_unavailable,
        project_boundaries: handler.boundaries.into_iter().collect(),
        partial_reasons,
        rate_limit,
    })
}

struct TreeHandler<'a, C, S> {
    cache: &'a C,
    analyzer_version: CacheVersion,
    rule_set_hash: RuleSetHash,
    limits: TreeCollectionLimits,
    sink: &'a mut S,
    observed_entries: usize,
    observed_blobs: usize,
    cache_hits: usize,
    cache_misses: usize,
    cache_unavailable: usize,
    boundaries: BTreeSet<String>,
    partial_reasons: BTreeSet<TreePartialReason>,
}

impl<C: BlobCacheLookup, S: TreeSink> TreeHandler<'_, C, S> {
    fn handle<E: de::Error>(&mut self, entry: TreeEntry) -> Result<(), E> {
        self.observed_entries = self
            .observed_entries
            .checked_add(1)
            .ok_or_else(|| E::custom("github_tree_entry_count_overflow"))?;
        if self.observed_entries > self.limits.max_entries() {
            self.partial_reasons.insert(TreePartialReason::EntryLimit);
            return Ok(());
        }
        if entry.kind != "blob" {
            return Ok(());
        }
        self.observed_blobs = self
            .observed_blobs
            .checked_add(1)
            .ok_or_else(|| E::custom("github_tree_blob_count_overflow"))?;
        if entry.path.len() > self.limits.max_path_bytes() || !is_safe_relative_path(&entry.path) {
            self.partial_reasons.insert(TreePartialReason::PathLimit);
            return Ok(());
        }
        self.observe_boundary(&entry.path);
        let blob = GitHubObjectId::from_str(&entry.sha)
            .map_err(|_| E::custom("github_tree_invalid_object_id"))?;
        let key = BlobAnalysisKey::new(
            blob.clone(),
            self.analyzer_version.clone(),
            self.rule_set_hash.clone(),
        );
        let cache_state = self.cache.lookup(&key);
        match cache_state {
            BlobCacheState::Hit => {
                self.cache_hits += 1;
                Ok(())
            }
            BlobCacheState::Miss => {
                self.cache_misses += 1;
                self.send(entry, blob, cache_state)
            }
            BlobCacheState::Unavailable => {
                self.cache_unavailable += 1;
                self.send(entry, blob, cache_state)
            }
        }
    }

    fn send<E: de::Error>(
        &mut self,
        entry: TreeEntry,
        blob: GitHubObjectId,
        cache_state: BlobCacheState,
    ) -> Result<(), E> {
        self.sink
            .accept(BlobWorkItem {
                path: entry.path,
                blob,
                size_bytes: entry.size,
                cache_state,
            })
            .map_err(|_| E::custom("github_tree_sink_failed"))
    }

    fn observe_boundary(&mut self, path: &str) {
        let Some(boundary) = project_boundary(path) else {
            return;
        };
        if self.boundaries.contains(boundary) {
            return;
        }
        if self.boundaries.len() >= self.limits.max_boundaries() {
            self.partial_reasons
                .insert(TreePartialReason::BoundaryLimit);
            return;
        }
        self.boundaries.insert(boundary.to_owned());
    }
}

#[derive(Deserialize)]
struct TreeEntry {
    path: String,
    #[serde(rename = "type")]
    kind: String,
    sha: String,
    #[serde(default)]
    size: Option<u64>,
    mode: String,
}

struct TreeEnvelope {
    truncated: bool,
}

struct TreeEnvelopeSeed<'handler, 'context, C, S> {
    handler: &'handler mut TreeHandler<'context, C, S>,
}

impl<'de, C: BlobCacheLookup, S: TreeSink> de::DeserializeSeed<'de>
    for TreeEnvelopeSeed<'_, '_, C, S>
{
    type Value = TreeEnvelope;

    fn deserialize<D: de::Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_map(TreeEnvelopeVisitor {
            handler: self.handler,
        })
    }
}

struct TreeEnvelopeVisitor<'handler, 'context, C, S> {
    handler: &'handler mut TreeHandler<'context, C, S>,
}

impl<'de, C: BlobCacheLookup, S: TreeSink> de::Visitor<'de> for TreeEnvelopeVisitor<'_, '_, C, S> {
    type Value = TreeEnvelope;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a bounded GitHub tree response")
    }

    fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut sha = None;
        let mut truncated = None;
        let mut tree_seen = false;
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "sha" if sha.is_none() => {
                    let value = map.next_value::<String>()?;
                    GitHubObjectId::from_str(&value)
                        .map_err(|_| de::Error::custom("github_tree_invalid_root_id"))?;
                    sha = Some(value);
                }
                "truncated" if truncated.is_none() => truncated = Some(map.next_value()?),
                "tree" if !tree_seen => {
                    map.next_value_seed(TreeEntriesSeed {
                        handler: self.handler,
                    })?;
                    tree_seen = true;
                }
                "sha" | "truncated" | "tree" => {
                    return Err(de::Error::custom("github_tree_duplicate_field"));
                }
                _ => {
                    map.next_value::<de::IgnoredAny>()?;
                }
            }
        }
        if sha.is_none() || !tree_seen {
            return Err(de::Error::custom("github_tree_missing_field"));
        }
        Ok(TreeEnvelope {
            truncated: truncated.ok_or_else(|| de::Error::custom("github_tree_missing_field"))?,
        })
    }
}

struct TreeEntriesSeed<'handler, 'context, C, S> {
    handler: &'handler mut TreeHandler<'context, C, S>,
}

impl<'de, C: BlobCacheLookup, S: TreeSink> de::DeserializeSeed<'de>
    for TreeEntriesSeed<'_, '_, C, S>
{
    type Value = ();

    fn deserialize<D: de::Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_seq(TreeEntriesVisitor {
            handler: self.handler,
        })
    }
}

struct TreeEntriesVisitor<'handler, 'context, C, S> {
    handler: &'handler mut TreeHandler<'context, C, S>,
}

impl<'de, C: BlobCacheLookup, S: TreeSink> de::Visitor<'de> for TreeEntriesVisitor<'_, '_, C, S> {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a sequence of GitHub tree entries")
    }

    fn visit_seq<A: de::SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
        while let Some(entry) = sequence.next_element::<TreeEntry>()? {
            let _mode_is_present = !entry.mode.is_empty();
            self.handler.handle(entry)?;
        }
        Ok(())
    }
}

fn is_safe_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\0')
        && path
            .split('/')
            .all(|component| !component.is_empty() && !matches!(component, "." | ".."))
}

fn project_boundary(path: &str) -> Option<&str> {
    let (directory, file_name) = path.rsplit_once('/').unwrap_or((".", path));
    let is_manifest = matches!(
        file_name,
        "package.json"
            | "pyproject.toml"
            | "Cargo.toml"
            | "go.mod"
            | "pom.xml"
            | "build.gradle"
            | "build.gradle.kts"
    ) || file_name.ends_with(".csproj");
    is_manifest.then_some(directory)
}

fn map_tree_parse_error(error: serde_json::Error) -> CollectionError {
    if error.to_string().contains("github_tree_sink_failed") {
        CollectionError::new(CollectionErrorKind::Sink, CollectionStage::Sink)
    } else if is_response_limit(&error) {
        CollectionError::new(CollectionErrorKind::ResponseLimit, CollectionStage::Tree)
    } else {
        CollectionError::new(
            CollectionErrorKind::InvalidProviderResponse,
            CollectionStage::Tree,
        )
    }
}
