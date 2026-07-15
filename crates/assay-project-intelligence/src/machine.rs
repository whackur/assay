use std::collections::{BTreeMap, BTreeSet};

use assay_domain::{AnalysisStatus, EvidenceStatus, RepositorySource};
use assay_git::{EntryMode, ObjectKind, RepositorySnapshot};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{
    ClassificationAvailabilityReason, ClassificationCategoryRecord,
    ClassificationEvidenceKindRecord, ClassificationEvidenceRecord, ClassificationTagRecord,
    PortablePathEncoding, ProjectEvidenceManifest, RawEvidenceFact, RawEvidenceIssue,
    RawEvidenceKind,
};

const PUBLIC_PATH_VALUE_LIMIT: usize = 8192;

/// Redacted deterministic mapping failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MachineMappingError;

/// Maps shared immutable facts into the reviewed public CLI bundle.
///
/// The supplied timestamp is a delivery-boundary clock value. No source bytes,
/// raw diffs, host paths, credentials, person observations, or scores are
/// retained.
pub fn build_project_analysis(
    snapshot: &RepositorySnapshot,
    manifest: &ProjectEvidenceManifest,
    generated_at: &str,
) -> Result<Value, MachineMappingError> {
    let classifications = manifest
        .classification_facts()
        .iter()
        .filter_map(|fact| fact.source_evidence_id().map(|id| (id.as_str(), fact)))
        .collect::<BTreeMap<_, _>>();
    let mut evidence = Vec::new();
    for raw in manifest.raw_facts() {
        evidence.push(map_raw(snapshot, raw, generated_at)?);
        if raw.kind() == RawEvidenceKind::TrackedFile
            && let Some(classification) = classifications.get(raw.id().as_str())
        {
            evidence.push(map_classification(classification, generated_at));
        }
    }
    evidence.extend(repository_features(snapshot, manifest, generated_at));
    evidence.sort_by(|left, right| left["id"].as_str().cmp(&right["id"].as_str()));

    let evidence_bytes = serde_json::to_vec(&evidence).map_err(|_| MachineMappingError)?;
    let artifact_hash = sha256(&evidence_bytes);
    let snapshot_id = manifest
        .raw_facts()
        .iter()
        .find(|fact| fact.kind() == RawEvidenceKind::RepositorySnapshot)
        .map(|fact| fact.id().as_str())
        .ok_or(MachineMappingError)?;
    let history = manifest
        .raw_facts()
        .iter()
        .find(|fact| fact.kind() == RawEvidenceKind::HistoryScope)
        .ok_or(MachineMappingError)?;
    let history_payload = history
        .payload()
        .as_history_scope()
        .ok_or(MachineMappingError)?;
    let has_bounded_path = manifest.raw_facts().iter().any(|fact| {
        fact.source()
            .path()
            .is_some_and(|path| !path_is_publishable(path))
    });
    let analysis_status = if has_bounded_path {
        "partial"
    } else {
        analysis_status(manifest.status())
    };
    let revision = snapshot.source_snapshot().revision().as_str();
    let source = map_repository(snapshot.source_snapshot().source());
    let warnings = warnings(manifest);
    let path_limit_ids = path_limit_ids(manifest);
    let attribute_unavailable_ids = evidence
        .iter()
        .filter(|fact| is_public_partial_attribute_classification(fact))
        .filter_map(|fact| fact["id"].as_str())
        .collect::<Vec<_>>();
    let mut limitations = Vec::new();
    if !attribute_unavailable_ids.is_empty() {
        limitations.push(json!({
            "code": "attribute_resolution_unavailable",
            "affected_evidence_ids": attribute_unavailable_ids
        }));
    }
    limitations.extend([
        json!({ "code": "project_scores_not_computed", "affected_evidence_ids": [snapshot_id] }),
        json!({ "code": "repository_code_not_executed", "affected_evidence_ids": [snapshot_id] }),
    ]);
    if !path_limit_ids.is_empty() {
        limitations
            .push(json!({ "code": "path_length_limit", "affected_evidence_ids": path_limit_ids }));
    }
    let manifest_value = json!({
        "schema_version": "1.0.0",
        "analysis_version": "repository-evidence-1",
        "tool": { "name": "assay", "version": env!("CARGO_PKG_VERSION") },
        "source_snapshot": {
            "source": source,
            "revision": revision,
            "root_tree": snapshot.source_snapshot().root_tree().map(|value| value.as_str()),
            "commit_time": snapshot.commit_time()
        },
        "rule_set_hash": stable_hash(b"assay-rule-set-v1\0classifier-v1\0project-evidence-v1"),
        "config_hash": stable_hash(b"assay-effective-config-v1\0default-local-read-only"),
        "analyzers": [
            { "name": "assay-classifier", "version": env!("CARGO_PKG_VERSION") },
            { "name": "assay-git", "version": env!("CARGO_PKG_VERSION") },
            { "name": "assay-project-intelligence", "version": env!("CARGO_PKG_VERSION") }
        ],
        "parsers": [],
        "status": analysis_status,
        "generated_at": generated_at,
        "scope": {
            "mode": "single_revision",
            "base_revision": Value::Null,
            "head_revision": revision,
            "history_status": evidence_status(history.status()),
            "commit_count": history_payload.reachable_commits(),
            "requested_capabilities": [
                "repository_snapshot", "tracked_files", "file_classification",
                "repository_history", "language_detection"
            ]
        },
        "data_sources": [
            {
                "id": snapshot_id,
                "kind": "repository",
                "status": evidence_status(snapshot.status()),
                "revision": revision,
                "content_hash": Value::Null,
                "remote_record_id": Value::Null,
                "collected_at": generated_at,
                "visibility": "private_local",
                "retention": "metadata_only"
            },
            {
                "id": history.id().as_str(),
                "kind": "repository_history",
                "status": evidence_status(history.status()),
                "revision": revision,
                "content_hash": Value::Null,
                "remote_record_id": Value::Null,
                "collected_at": generated_at,
                "visibility": "private_local",
                "retention": "metadata_only"
            }
        ],
        "artifacts": [{
            "role": "project_evidence",
            "schema_version": "1.0.0",
            "content_hash": artifact_hash,
            "record_count": evidence.len(),
            "status": analysis_status
        }],
        "warnings": warnings,
        "limitations": limitations
    });
    Ok(json!({
        "schema_version": "1.0.0",
        "manifest": manifest_value,
        "evidence": evidence
    }))
}

fn map_raw(
    snapshot: &RepositorySnapshot,
    fact: &RawEvidenceFact,
    collected_at: &str,
) -> Result<Value, MachineMappingError> {
    if matches!(
        fact.kind(),
        RawEvidenceKind::HistoryScope | RawEvidenceKind::ParentDelta
    ) && !matches!(
        fact.status(),
        EvidenceStatus::Complete | EvidenceStatus::Partial
    ) {
        let issue = match fact.kind() {
            RawEvidenceKind::HistoryScope => fact
                .payload()
                .as_history_scope()
                .and_then(|value| value.issue()),
            RawEvidenceKind::ParentDelta => fact
                .payload()
                .as_parent_delta()
                .and_then(|value| value.issue()),
            _ => None,
        };
        return Ok(json!({
            "schema_version": "1.0.0",
            "repository": map_repository(fact.source().repository()),
            "id": fact.id().as_str(),
            "status": evidence_status(fact.status()),
            "grade": Value::Null,
            "privacy": privacy(),
            "requested_kind": if fact.kind() == RawEvidenceKind::HistoryScope { "history_scope" } else { "parent_delta" },
            "reason": issue.map(raw_issue).unwrap_or("source_unavailable")
        }));
    }
    let payload = match fact.kind() {
        RawEvidenceKind::RepositorySnapshot => json!({
            "kind": "repository_snapshot",
            "commit_time": snapshot.commit_time(),
            "root_tree": snapshot.source_snapshot().root_tree().map(|value| value.as_str())
        }),
        RawEvidenceKind::TrackedFile => {
            let file = fact
                .payload()
                .as_tracked_file()
                .ok_or(MachineMappingError)?;
            let path = fact.source().path().ok_or(MachineMappingError)?;
            if !path_is_publishable(path) {
                return Ok(availability_envelope(
                    fact,
                    "unsupported",
                    "tracked_file",
                    "path_length_limit",
                ));
            }
            let (language, language_status) = language(path.encoding(), path.value());
            json!({
                "kind": "tracked_file",
                "path": { "encoding": path_encoding(path.encoding()), "value": path.value() },
                "mode": entry_mode(file.mode()),
                "object_kind": object_kind(file.object_kind()),
                "object_id": fact.source().object_id().ok_or(MachineMappingError)?,
                "content_status": evidence_status(fact.status()),
                "language": language,
                "language_status": language_status,
                "size_bytes": file.size_bytes(),
                "content_hash": file.content_hash().map(|hash| hash.as_str()),
                "issue": file.issue().map(raw_issue)
            })
        }
        RawEvidenceKind::HistoryScope => {
            let history = fact
                .payload()
                .as_history_scope()
                .ok_or(MachineMappingError)?;
            json!({
                "kind": "history_scope",
                "base_revision": Value::Null,
                "head_revision": snapshot.source_snapshot().revision().as_str(),
                "commit_count": history.reachable_commits()
            })
        }
        RawEvidenceKind::ParentDelta => {
            let delta = fact
                .payload()
                .as_parent_delta()
                .ok_or(MachineMappingError)?;
            json!({
                "kind": "parent_delta",
                "changed_entries": delta.changed_entries(),
                "renames": delta.renames(),
                "issue": delta.issue().map(raw_issue)
            })
        }
    };
    let published_status = if fact.kind() == RawEvidenceKind::TrackedFile
        && !matches!(
            fact.status(),
            EvidenceStatus::Complete | EvidenceStatus::Partial
        ) {
        "partial"
    } else {
        evidence_status(fact.status())
    };
    Ok(factual_record(
        fact,
        collected_at,
        published_status,
        payload,
    ))
}

fn factual_record(
    fact: &RawEvidenceFact,
    collected_at: &str,
    status: &'static str,
    payload: Value,
) -> Value {
    json!({
        "schema_version": "1.0.0",
        "repository": map_repository(fact.source().repository()),
        "id": fact.id().as_str(),
        "status": status,
        "grade": "a",
        "privacy": privacy(),
        "provenance": {
            "source_kind": source_kind(fact.kind()),
            "collected_at": collected_at,
            "repository_revision": fact.source().repository_revision().as_str(),
            "content_hash": fact.content_hash().map(|hash| hash.as_str()),
            "remote_record_id": Value::Null
        },
        "payload": payload
    })
}

fn availability_envelope(
    fact: &RawEvidenceFact,
    status: &'static str,
    requested_kind: &'static str,
    reason: &'static str,
) -> Value {
    json!({
        "schema_version": "1.0.0",
        "repository": map_repository(fact.source().repository()),
        "id": fact.id().as_str(),
        "status": status,
        "grade": Value::Null,
        "privacy": privacy(),
        "requested_kind": requested_kind,
        "reason": reason
    })
}

fn map_classification(fact: &ClassificationEvidenceRecord, collected_at: &str) -> Value {
    let related = fact
        .source_evidence_id()
        .map(|id| vec![id.as_str()])
        .unwrap_or_default();
    let mut common = json!({
        "schema_version": "1.0.0",
        "repository": map_repository(fact.source().repository()),
        "id": fact.id().as_str(),
        "status": evidence_status(fact.status()),
        "grade": if matches!(fact.status(), EvidenceStatus::Complete | EvidenceStatus::Partial) { Value::String("a".into()) } else { Value::Null },
        "privacy": privacy(),
        "related_evidence_ids": related
    });
    if let Some(policy_version) = fact.policy_version() {
        common["attempted_policy_version"] = Value::String(policy_version.to_owned());
    }
    let path_limited = fact
        .source()
        .path()
        .is_some_and(|path| !path_is_publishable(path));
    if path_limited && fact.policy_version().is_some() {
        common["status"] = Value::String("unsupported".into());
        common["grade"] = Value::Null;
        common["requested_kind"] = Value::String("file_classification".into());
        common["reason"] = Value::String("path_length_limit".into());
        return common;
    }
    if matches!(
        fact.status(),
        EvidenceStatus::Complete | EvidenceStatus::Partial
    ) {
        let mut value = common;
        value["provenance"] = json!({
            "source_kind": "repository_content",
            "collected_at": collected_at,
            "repository_revision": fact.source().repository_revision().as_str(),
            "content_hash": Value::Null,
            "remote_record_id": Value::Null
        });
        value["payload"] = json!({
            "kind": "file_classification",
            "source_evidence_id": fact.source_evidence_id().map(|id| id.as_str()),
            "policy_version": fact.policy_version(),
            "reason": fact.reason().map(classification_reason),
            "classification": {
                "primary_category": fact.category().map(category),
                "tags": fact.tags().iter().copied().filter_map(tag).collect::<Vec<_>>(),
                "rule_id": fact.rule_id(),
                "confidence": fact.confidence_basis_points().map(|value| f64::from(value) / 10_000.0),
                "evidence": fact.classification_evidence().iter().map(|item| json!({
                    "kind": classification_evidence_kind(item.kind()),
                    "rule_id": item.rule_id(),
                    "attribute_name": item.attribute_name(),
                    "attribute_value": item.attribute_value()
                })).collect::<Vec<_>>()
            }
        });
        value
    } else {
        let mut value = common;
        value["requested_kind"] = Value::String("file_classification".into());
        value["reason"] = Value::String(
            fact.reason()
                .map(classification_reason)
                .unwrap_or("missing_classification")
                .into(),
        );
        value
    }
}

fn repository_features(
    snapshot: &RepositorySnapshot,
    manifest: &ProjectEvidenceManifest,
    collected_at: &str,
) -> Vec<Value> {
    let raw_files = manifest
        .raw_facts()
        .iter()
        .filter(|fact| fact.kind() == RawEvidenceKind::TrackedFile)
        .collect::<Vec<_>>();
    let classification_by_raw = manifest
        .classification_facts()
        .iter()
        .filter_map(|fact| fact.source_evidence_id().map(|id| (id.as_str(), fact)))
        .collect::<BTreeMap<_, _>>();
    let incomplete_classification_ids = manifest
        .classification_facts()
        .iter()
        .filter(|fact| !classification_is_publicly_complete(fact))
        .map(|fact| fact.id().as_str())
        .collect::<BTreeSet<_>>();
    let incomplete_raw_path_ids = raw_files
        .iter()
        .filter(|fact| {
            fact.source().path().is_some_and(|path| {
                path.encoding() != PortablePathEncoding::Utf8 || !path_is_publishable(path)
            })
        })
        .map(|fact| fact.id().as_str())
        .collect::<BTreeSet<_>>();
    let features = [
        "readme",
        "license",
        "package_manifest",
        "ci",
        "test",
        "documentation",
        "migration",
        "dependency",
        "security_policy",
        "generated_content",
        "vendored_content",
    ];
    features
        .into_iter()
        .map(|feature| {
            let path_only = matches!(feature, "readme" | "license" | "package_manifest");
            let mut reliable = BTreeSet::new();
            let mut candidates = BTreeSet::new();
            for raw in &raw_files {
                let path = raw.source().path();
                let classification = classification_by_raw.get(raw.id().as_str()).copied();
                if matches_feature(feature, path.map(|path| path.value()), classification) {
                    if path_only {
                        if path.is_some_and(path_is_publishable) {
                            reliable.insert(raw.id().as_str());
                        } else {
                            candidates.insert(raw.id().as_str());
                        }
                    } else if let Some(classification) = classification {
                        if classification_is_publicly_complete(classification) {
                            reliable.insert(classification.id().as_str());
                        } else {
                            candidates.insert(classification.id().as_str());
                        }
                    }
                }
            }
            if reliable.is_empty() && candidates.is_empty() {
                candidates.extend(if path_only {
                    incomplete_raw_path_ids.iter().copied()
                } else {
                    incomplete_classification_ids.iter().copied()
                });
            }
            let state = if !reliable.is_empty() {
                "present"
            } else if !candidates.is_empty() {
                "unavailable"
            } else {
                "absent"
            };
            let status = if state == "unavailable" {
                "partial"
            } else {
                "complete"
            };
            let related = if reliable.is_empty() {
                candidates
            } else {
                reliable
            };
            let identity_scope = repository_identity_component(snapshot.source_snapshot().source());
            let id_input = format!(
                "{identity_scope}\0{}\0{feature}\0{state}\0{}",
                snapshot.source_snapshot().revision().as_str(),
                related.iter().copied().collect::<Vec<_>>().join("\0")
            );
            let id = format!(
                "evidence:repository-feature:v1-{}",
                &stable_hex(id_input.as_bytes())[..24]
            );
            json!({
                "schema_version": "1.0.0",
                "repository": map_repository(snapshot.source_snapshot().source()),
                "id": id,
                "status": status,
                "grade": "a",
                "privacy": privacy(),
                "provenance": {
                    "source_kind": "repository_content",
                    "collected_at": collected_at,
                    "repository_revision": snapshot.source_snapshot().revision().as_str(),
                    "content_hash": Value::Null,
                    "remote_record_id": Value::Null
                },
                "payload": {
                    "kind": "repository_feature",
                    "feature": feature,
                    "state": state,
                    "related_evidence_ids": related.into_iter().collect::<Vec<_>>()
                }
            })
        })
        .collect()
}

fn matches_feature(
    feature: &str,
    path: Option<&str>,
    classification: Option<&ClassificationEvidenceRecord>,
) -> bool {
    let lower = path.unwrap_or_default().to_ascii_lowercase();
    match feature {
        "readme" => lower
            .rsplit('/')
            .next()
            .is_some_and(|name| name.starts_with("readme")),
        "license" => lower
            .rsplit('/')
            .next()
            .is_some_and(|name| name.starts_with("license") || name == "copying"),
        "package_manifest" => matches!(
            lower.rsplit('/').next(),
            Some("package.json" | "pyproject.toml" | "setup.py" | "setup.cfg")
        ),
        "ci" => {
            classification.and_then(|fact| fact.category())
                == Some(ClassificationCategoryRecord::CiCd)
        }
        "test" => {
            classification.and_then(|fact| fact.category())
                == Some(ClassificationCategoryRecord::Test)
        }
        "documentation" => {
            classification.and_then(|fact| fact.category())
                == Some(ClassificationCategoryRecord::Documentation)
        }
        "migration" => {
            classification.and_then(|fact| fact.category())
                == Some(ClassificationCategoryRecord::SchemaMigration)
        }
        "dependency" => {
            classification.and_then(|fact| fact.category())
                == Some(ClassificationCategoryRecord::Dependency)
        }
        "security_policy" => {
            classification.and_then(|fact| fact.category())
                == Some(ClassificationCategoryRecord::SecurityPolicy)
        }
        "generated_content" => {
            classification.and_then(|fact| fact.category())
                == Some(ClassificationCategoryRecord::Generated)
        }
        "vendored_content" => {
            classification.and_then(|fact| fact.category())
                == Some(ClassificationCategoryRecord::Vendored)
        }
        _ => false,
    }
}

fn warnings(manifest: &ProjectEvidenceManifest) -> Vec<Value> {
    manifest
        .raw_facts()
        .iter()
        .filter_map(|fact| {
            let issue = match fact.kind() {
                RawEvidenceKind::TrackedFile => {
                    fact.payload().as_tracked_file().and_then(|p| p.issue())
                }
                RawEvidenceKind::HistoryScope => {
                    fact.payload().as_history_scope().and_then(|p| p.issue())
                }
                RawEvidenceKind::ParentDelta => {
                    fact.payload().as_parent_delta().and_then(|p| p.issue())
                }
                RawEvidenceKind::RepositorySnapshot => None,
            }?;
            Some(json!({ "code": raw_issue(issue), "affected_evidence_ids": [fact.id().as_str()] }))
        })
        .collect()
}

fn path_limit_ids(manifest: &ProjectEvidenceManifest) -> Vec<&str> {
    let limited_raw = manifest
        .raw_facts()
        .iter()
        .filter(|fact| {
            fact.source()
                .path()
                .is_some_and(|path| !path_is_publishable(path))
        })
        .map(|fact| fact.id().as_str())
        .collect::<BTreeSet<_>>();
    let mut ids = limited_raw.iter().copied().collect::<BTreeSet<_>>();
    ids.extend(
        manifest
            .classification_facts()
            .iter()
            .filter(|fact| {
                fact.source_evidence_id()
                    .is_some_and(|id| limited_raw.contains(id.as_str()))
            })
            .map(|fact| fact.id().as_str()),
    );
    ids.into_iter().collect()
}

fn path_is_publishable(path: &crate::PortableRepositoryPath) -> bool {
    path.value().chars().count() <= PUBLIC_PATH_VALUE_LIMIT
}

fn classification_is_publicly_complete(fact: &ClassificationEvidenceRecord) -> bool {
    fact.status() == EvidenceStatus::Complete
        && fact.source().path().is_some_and(path_is_publishable)
}

fn is_public_partial_attribute_classification(fact: &Value) -> bool {
    fact["status"] == "partial"
        && fact["payload"]["kind"] == "file_classification"
        && fact["payload"]["reason"] == "attributes_unavailable"
}

fn privacy() -> Value {
    json!({ "visibility": "private_local", "source_content": "not_retained", "external_transmission": "prohibited" })
}
fn map_repository(source: &RepositorySource) -> Value {
    if let Some(id) = source.local_repository_id() {
        json!({ "kind": "local", "repository_id": id.as_str() })
    } else if let Some((provider, namespace, repository)) = source.hosted_locator() {
        json!({ "kind": "hosted", "provider": provider, "namespace": namespace, "repository": repository })
    } else {
        unreachable!("repository source variants are closed")
    }
}
fn repository_identity_component(source: &RepositorySource) -> String {
    if let Some(id) = source.local_repository_id() {
        format!("local:{}", id.as_str())
    } else if let Some((provider, namespace, repository)) = source.hosted_locator() {
        format!("hosted:{provider}:{namespace}:{repository}")
    } else {
        unreachable!("repository source variants are closed")
    }
}
fn source_kind(kind: RawEvidenceKind) -> &'static str {
    match kind {
        RawEvidenceKind::RepositorySnapshot => "repository",
        RawEvidenceKind::TrackedFile => "repository_content",
        RawEvidenceKind::HistoryScope | RawEvidenceKind::ParentDelta => "repository_history",
    }
}
fn evidence_status(status: EvidenceStatus) -> &'static str {
    match status {
        EvidenceStatus::Complete => "complete",
        EvidenceStatus::Partial => "partial",
        EvidenceStatus::Unavailable => "unavailable",
        EvidenceStatus::Unsupported => "unsupported",
        EvidenceStatus::Insufficient => "insufficient",
        EvidenceStatus::Pending => "pending",
    }
}
fn analysis_status(status: AnalysisStatus) -> &'static str {
    match status {
        AnalysisStatus::Complete => "complete",
        AnalysisStatus::Partial => "partial",
        AnalysisStatus::Unavailable => "unavailable",
        AnalysisStatus::Unsupported => "unsupported",
        AnalysisStatus::Insufficient => "insufficient",
        AnalysisStatus::Pending => "pending",
    }
}
fn path_encoding(value: PortablePathEncoding) -> &'static str {
    match value {
        PortablePathEncoding::Utf8 => "utf8",
        PortablePathEncoding::GitPathHex => "git_path_hex",
    }
}
fn entry_mode(value: EntryMode) -> &'static str {
    match value {
        EntryMode::Regular => "regular",
        EntryMode::Executable => "executable",
        EntryMode::SymbolicLink => "symbolic_link",
        EntryMode::Gitlink => "gitlink",
    }
}
fn object_kind(value: ObjectKind) -> &'static str {
    match value {
        ObjectKind::Blob => "blob",
        ObjectKind::Commit => "commit",
    }
}
fn language(encoding: PortablePathEncoding, path: &str) -> (Option<&'static str>, &'static str) {
    if encoding != PortablePathEncoding::Utf8 {
        return (None, "unsupported");
    }
    match path
        .rsplit('.')
        .next()
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("js" | "mjs" | "cjs") => (Some("JavaScript"), "complete"),
        Some("ts") => (Some("TypeScript"), "complete"),
        Some("tsx") => (Some("TSX"), "complete"),
        Some("py") => (Some("Python"), "complete"),
        _ => (None, "unsupported"),
    }
}
fn raw_issue(value: RawEvidenceIssue) -> &'static str {
    match value {
        RawEvidenceIssue::GitlinkContent => "gitlink_content",
        RawEvidenceIssue::SizeLimit => "size_limit",
        RawEvidenceIssue::MissingOrUnreadable => "missing_or_unreadable",
        RawEvidenceIssue::Timeout => "timeout",
        RawEvidenceIssue::OutputLimit => "output_limit",
        RawEvidenceIssue::MalformedMetadata => "malformed_metadata",
        RawEvidenceIssue::HistoryDepthLimit => "history_depth_limit",
        RawEvidenceIssue::ShallowRepository => "shallow_repository",
        RawEvidenceIssue::ProcessFailure => "process_failure",
        RawEvidenceIssue::MalformedOutput => "malformed_output",
        RawEvidenceIssue::RenameCandidateLimit => "rename_candidate_limit",
    }
}
fn classification_reason(value: ClassificationAvailabilityReason) -> &'static str {
    match value {
        ClassificationAvailabilityReason::AttributesUnavailable => "attributes_unavailable",
        ClassificationAvailabilityReason::MissingClassification => "missing_classification",
        ClassificationAvailabilityReason::NonPortablePath => "non_portable_path",
    }
}
fn category(value: ClassificationCategoryRecord) -> &'static str {
    match value {
        ClassificationCategoryRecord::ProductionCode => "production_code",
        ClassificationCategoryRecord::Test => "test",
        ClassificationCategoryRecord::Documentation => "documentation",
        ClassificationCategoryRecord::CiCd => "ci_cd",
        ClassificationCategoryRecord::Infrastructure => "infrastructure",
        ClassificationCategoryRecord::SchemaMigration => "schema_migration",
        ClassificationCategoryRecord::Dependency => "dependency",
        ClassificationCategoryRecord::SecurityPolicy => "security",
        ClassificationCategoryRecord::Configuration => "configuration",
        ClassificationCategoryRecord::Generated => "generated",
        ClassificationCategoryRecord::Vendored => "vendored",
        ClassificationCategoryRecord::BuildOutput => "build_output",
        ClassificationCategoryRecord::Coverage => "coverage",
        ClassificationCategoryRecord::Unknown => "unknown",
    }
}
fn tag(value: ClassificationTagRecord) -> Option<&'static str> {
    match value {
        ClassificationTagRecord::DependencyManifest => Some("dependency"),
        ClassificationTagRecord::Lockfile => Some("lockfile"),
        ClassificationTagRecord::LinguistGenerated => Some("generated"),
        ClassificationTagRecord::LinguistVendored => Some("vendored"),
        ClassificationTagRecord::Minified => Some("minified"),
        ClassificationTagRecord::GeneratedSuppressed
        | ClassificationTagRecord::VendoredSuppressed
        | ClassificationTagRecord::AttributesUnavailable => None,
    }
}
fn classification_evidence_kind(value: ClassificationEvidenceKindRecord) -> &'static str {
    match value {
        ClassificationEvidenceKindRecord::PolicyRule => "policy_rule",
        ClassificationEvidenceKindRecord::LinguistAttribute => "linguist_attribute",
        ClassificationEvidenceKindRecord::AttributeFactsUnavailable => {
            "attribute_facts_unavailable"
        }
    }
}
fn stable_hash(bytes: &[u8]) -> String {
    format!("sha256:{}", stable_hex(bytes))
}
fn stable_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn sha256(bytes: &[u8]) -> String {
    stable_hash(bytes)
}
