use std::collections::BTreeMap;

use assay_domain::{AnalysisStatus, EvidenceId, EvidenceStatus};
use assay_git::{RepositorySnapshot, TrackedEntry};

use crate::evidence::classification_record::{
    ClassificationEvidenceRecord, ClassifiedSnapshotFile,
};
use crate::evidence::codes::{classification_reason_code, evidence_status_code};
use crate::evidence::error::{EvidenceAssemblyError, EvidenceAssemblyErrorKind};
use crate::evidence::id::{
    EvidenceIdBuilder, add_classification_payload, add_raw_payload, add_snapshot_scope,
    portable_path_bytes,
};
use crate::evidence::manifest::ProjectEvidenceManifest;
use crate::evidence::mapping::{
    map_history_issue, map_object_issue, map_parent_delta_issue, parent_delta_values,
};
use crate::evidence::payload::RawEvidencePayload;
use crate::evidence::raw_fact::RawEvidenceFact;
use crate::evidence::source::EvidenceSourceRecord;
use crate::evidence::types::{ClassificationAvailabilityReason, RawEvidenceKind};

/// Combines one immutable snapshot with zero or more bound classification facts.
///
/// A missing classification becomes an explicit unavailable, citable record.
/// Duplicate and foreign bindings fail closed. All supplied classifications,
/// including unsupported attempts,
/// must use one policy version.
pub fn assemble_project_evidence(
    snapshot: &RepositorySnapshot,
    classifications: impl IntoIterator<Item = ClassifiedSnapshotFile>,
) -> Result<ProjectEvidenceManifest, EvidenceAssemblyError> {
    let mut raw_facts = raw_facts(snapshot)?;
    let raw_files = raw_file_lookup(&raw_facts);
    let snapshot_keys = snapshot
        .entries()
        .iter()
        .map(|entry| (entry_key(entry), ()))
        .collect::<BTreeMap<_, _>>();

    let mut supplied = BTreeMap::new();
    let mut policy_version: Option<String> = None;
    for classification in classifications {
        if !classification.snapshot.matches(snapshot) {
            return Err(EvidenceAssemblyError::new(
                EvidenceAssemblyErrorKind::ClassificationSnapshotMismatch,
            ));
        }
        let key = classification.key();
        if !snapshot_keys.contains_key(&key) {
            return Err(EvidenceAssemblyError::new(
                EvidenceAssemblyErrorKind::ClassificationSnapshotMismatch,
            ));
        }
        if let Some(expected) = &policy_version {
            if expected != &classification.attempted_policy_version {
                return Err(EvidenceAssemblyError::new(
                    EvidenceAssemblyErrorKind::MixedClassificationPolicy,
                ));
            }
        } else {
            policy_version = Some(classification.attempted_policy_version.clone());
        }
        if supplied.insert(key, classification).is_some() {
            return Err(EvidenceAssemblyError::new(
                EvidenceAssemblyErrorKind::DuplicateClassification,
            ));
        }
    }

    let mut classification_facts = Vec::with_capacity(snapshot.entries().len());
    for entry in snapshot.entries() {
        let key = entry_key(entry);
        let raw = raw_files.get(&key).ok_or_else(|| {
            EvidenceAssemblyError::new(EvidenceAssemblyErrorKind::EvidenceIdCollision)
        })?;
        let source = EvidenceSourceRecord::entry(snapshot, entry);
        let bound = supplied.remove(&key);
        classification_facts.push(classification_fact(raw.id(), source, bound)?);
    }
    if !supplied.is_empty() {
        return Err(EvidenceAssemblyError::new(
            EvidenceAssemblyErrorKind::ClassificationSnapshotMismatch,
        ));
    }

    raw_facts.sort_by(|left, right| left.id.cmp(&right.id));
    classification_facts.sort_by(|left, right| left.id.cmp(&right.id));
    reject_duplicate_ids(
        raw_facts.iter().map(RawEvidenceFact::id).chain(
            classification_facts
                .iter()
                .map(ClassificationEvidenceRecord::id),
        ),
    )?;

    let status = if snapshot.status() == EvidenceStatus::Complete
        && classification_facts
            .iter()
            .all(|fact| fact.status == EvidenceStatus::Complete)
    {
        AnalysisStatus::Complete
    } else {
        AnalysisStatus::Partial
    };

    Ok(ProjectEvidenceManifest {
        status,
        classification_policy_version: policy_version,
        raw_facts,
        classification_facts,
    })
}

fn raw_facts(snapshot: &RepositorySnapshot) -> Result<Vec<RawEvidenceFact>, EvidenceAssemblyError> {
    let repository_source = EvidenceSourceRecord::for_repository(snapshot);
    let mut facts = Vec::with_capacity(snapshot.entries().len() + 3);

    let snapshot_payload = RawEvidencePayload::repository_snapshot();
    facts.push(raw_fact(
        snapshot,
        RawEvidenceKind::RepositorySnapshot,
        snapshot.status(),
        repository_source.clone(),
        snapshot_payload,
    )?);

    for entry in snapshot.entries() {
        let payload = RawEvidencePayload::tracked_file(
            entry.mode(),
            entry.kind(),
            entry.content().size(),
            entry.content().content_hash().cloned(),
            entry.content().issue().map(map_object_issue),
        );
        facts.push(raw_fact(
            snapshot,
            RawEvidenceKind::TrackedFile,
            entry.content().status(),
            EvidenceSourceRecord::entry(snapshot, entry),
            payload,
        )?);
    }

    let history_usable = matches!(
        snapshot.history().status(),
        EvidenceStatus::Complete | EvidenceStatus::Partial
    );
    let history = RawEvidencePayload::history_scope(
        history_usable.then_some(snapshot.history().reachable_commits()),
        history_usable.then_some(snapshot.history().truncated()),
        snapshot.history().issue().map(map_history_issue),
    );
    facts.push(raw_fact(
        snapshot,
        RawEvidenceKind::HistoryScope,
        snapshot.history().status(),
        repository_source.clone(),
        history,
    )?);

    let (changed_entries, renames) = parent_delta_values(
        snapshot.parent_delta().status(),
        snapshot.parent_delta().issue(),
        snapshot.parent_delta().changed_entries(),
        snapshot.parent_delta().renames(),
    );
    let delta = RawEvidencePayload::parent_delta(
        changed_entries,
        renames,
        snapshot.parent_delta().issue().map(map_parent_delta_issue),
    );
    facts.push(raw_fact(
        snapshot,
        RawEvidenceKind::ParentDelta,
        snapshot.parent_delta().status(),
        repository_source,
        delta,
    )?);
    Ok(facts)
}

fn raw_fact(
    snapshot: &RepositorySnapshot,
    kind: RawEvidenceKind,
    status: EvidenceStatus,
    source: EvidenceSourceRecord,
    payload: RawEvidencePayload,
) -> Result<RawEvidenceFact, EvidenceAssemblyError> {
    let mut id = EvidenceIdBuilder::new(kind.id_component());
    add_snapshot_scope(&mut id, snapshot);
    id.optional_field(
        b"path_bytes",
        source
            .path
            .as_ref()
            .and_then(portable_path_bytes)
            .as_deref(),
    );
    id.optional_field(b"object_id", source.object_id.as_deref().map(str::as_bytes));
    id.field(b"status", evidence_status_code(status).as_bytes());
    add_raw_payload(&mut id, &payload);
    Ok(RawEvidenceFact {
        id: id.finish(kind.id_component())?,
        kind,
        status,
        source,
        payload,
    })
}

fn classification_fact(
    raw_id: &EvidenceId,
    source: EvidenceSourceRecord,
    classification: Option<ClassifiedSnapshotFile>,
) -> Result<ClassificationEvidenceRecord, EvidenceAssemblyError> {
    let (status, attempted_policy_version, reason, payload) = classification.map_or(
        (
            EvidenceStatus::Unavailable,
            None,
            Some(ClassificationAvailabilityReason::MissingClassification),
            None,
        ),
        |value| {
            (
                value.status,
                Some(value.attempted_policy_version),
                value.reason,
                value.payload,
            )
        },
    );
    let mut id = EvidenceIdBuilder::new("file-classification");
    id.field(b"raw_evidence_id", raw_id.as_str().as_bytes());
    id.field(b"status", evidence_status_code(status).as_bytes());
    id.optional_field(
        b"attempted_policy_version",
        attempted_policy_version.as_deref().map(str::as_bytes),
    );
    id.optional_field(
        b"reason",
        reason.map(classification_reason_code).map(str::as_bytes),
    );
    if let Some(payload) = &payload {
        add_classification_payload(&mut id, payload);
    }
    Ok(ClassificationEvidenceRecord {
        id: id.finish("file-classification")?,
        status,
        source_evidence_id: raw_id.clone(),
        source,
        attempted_policy_version,
        reason,
        payload,
    })
}

fn raw_file_lookup(facts: &[RawEvidenceFact]) -> BTreeMap<(Vec<u8>, String), &RawEvidenceFact> {
    facts
        .iter()
        .filter_map(|fact| {
            if fact.kind != RawEvidenceKind::TrackedFile {
                return None;
            }
            let path = fact.source.path.as_ref()?;
            let bytes = portable_path_bytes(path)?;
            Some(((bytes, fact.source.object_id.clone()?), fact))
        })
        .collect()
}

fn reject_duplicate_ids<'a>(
    ids: impl IntoIterator<Item = &'a EvidenceId>,
) -> Result<(), EvidenceAssemblyError> {
    let mut values = ids.into_iter().map(EvidenceId::as_str).collect::<Vec<_>>();
    values.sort_unstable();
    if values.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(EvidenceAssemblyError::new(
            EvidenceAssemblyErrorKind::EvidenceIdCollision,
        ));
    }
    Ok(())
}

fn entry_key(entry: &TrackedEntry) -> (Vec<u8>, String) {
    (
        entry.path().as_bytes().to_vec(),
        entry.object_id().as_str().to_owned(),
    )
}
