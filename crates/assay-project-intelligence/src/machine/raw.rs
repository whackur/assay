use assay_domain::EvidenceStatus;
use assay_git::RepositorySnapshot;
use serde_json::{Value, json};

use crate::PortableRepositoryPath;
use crate::RawEvidenceFact;
use crate::RawEvidenceKind;
use crate::machine::codes::{
    entry_mode, evidence_status, language, object_kind, path_encoding, raw_issue, source_kind,
};
use crate::machine::error::MachineMappingError;
use crate::machine::repository::{map_repository, privacy};

pub(crate) fn map_raw(
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

pub(crate) fn path_is_publishable(path: &PortableRepositoryPath) -> bool {
    path.value().chars().count() <= PUBLIC_PATH_VALUE_LIMIT
}

pub(crate) const PUBLIC_PATH_VALUE_LIMIT: usize = 8192;
