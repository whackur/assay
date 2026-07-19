use std::{collections::BTreeSet, str::FromStr};

use assay_domain::RuleSetHash;
use serde::{Deserialize, de::DeserializeSeed as _};

use crate::{
    BlobAnalysisKey, BlobCacheLookup, BlobCacheState, CacheVersion, CollectionError,
    CollectionErrorKind, CollectionStage, GitHubObjectId, GitHubResponse, RateLimitState,
    collection::{LimitedReader, content_length_exceeds, is_response_limit},
    tree::{
        contract::{BlobWorkItem, TreeCollectionSummary, TreeSink},
        limits::{CollectionStatus, TreeCollectionLimits, TreePartialReason},
        path::{is_safe_relative_path, project_boundary},
        visitor::TreeEnvelopeSeed,
    },
};

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

pub(crate) struct TreeHandler<'a, C, S> {
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
    pub(crate) fn handle<E: serde::de::Error>(&mut self, entry: TreeEntry) -> Result<(), E> {
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

    fn send<E: serde::de::Error>(
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
pub(crate) struct TreeEntry {
    pub(crate) path: String,
    #[serde(rename = "type")]
    pub(crate) kind: String,
    pub(crate) sha: String,
    #[serde(default)]
    pub(crate) size: Option<u64>,
    pub(crate) mode: String,
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
