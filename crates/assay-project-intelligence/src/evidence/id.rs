use std::str::FromStr;

use assay_domain::{ContentHash, EvidenceId, RepositorySource, RevisionId};
use assay_git::{GitObjectFormat, RepositorySnapshot};
use sha2::{Digest, Sha256};

use crate::evidence::classification_record::ClassificationPayload;
use crate::evidence::codes::{
    classification_category_code, classification_evidence_kind_code, classification_tag_code,
    entry_mode_code, object_kind_code, raw_issue_code,
};
use crate::evidence::error::{EvidenceAssemblyError, EvidenceAssemblyErrorKind};
use crate::evidence::hex::decode_hex;
use crate::evidence::payload::{RawEvidencePayload, RawEvidencePayloadData};
use crate::evidence::types::{PortablePathEncoding, PortableRepositoryPath};

pub(crate) const EVIDENCE_ID_DOMAIN: &[u8] = b"assay.project-intelligence.evidence-id.v1";

pub(crate) struct EvidenceIdBuilder(Sha256);

impl EvidenceIdBuilder {
    pub(crate) fn new(kind: &str) -> Self {
        let mut hash = Sha256::new();
        update_length_prefixed(&mut hash, EVIDENCE_ID_DOMAIN);
        update_length_prefixed(&mut hash, kind.as_bytes());
        Self(hash)
    }

    pub(crate) fn field(&mut self, name: &[u8], value: &[u8]) {
        update_length_prefixed(&mut self.0, name);
        update_length_prefixed(&mut self.0, value);
    }

    pub(crate) fn optional_field(&mut self, name: &[u8], value: Option<&[u8]>) {
        self.field(name, if value.is_some() { b"some" } else { b"none" });
        if let Some(value) = value {
            self.field(b"value", value);
        }
    }

    pub(crate) fn finish(self, kind: &str) -> Result<EvidenceId, EvidenceAssemblyError> {
        let digest = crate::evidence::hex::lower_hex(&self.0.finalize());
        EvidenceId::from_str(&format!("evidence:{kind}:v1-{digest}")).map_err(|_| {
            EvidenceAssemblyError::new(EvidenceAssemblyErrorKind::EvidenceIdGeneration)
        })
    }
}

fn update_length_prefixed(hash: &mut Sha256, value: &[u8]) {
    hash.update((value.len() as u64).to_be_bytes());
    hash.update(value);
}

pub(crate) fn add_snapshot_scope(id: &mut EvidenceIdBuilder, snapshot: &RepositorySnapshot) {
    add_repository_source(id, snapshot.source_snapshot().source());
    id.field(
        b"revision",
        snapshot.source_snapshot().revision().as_str().as_bytes(),
    );
    id.optional_field(
        b"root_tree",
        snapshot
            .source_snapshot()
            .root_tree()
            .map(RevisionId::as_str)
            .map(str::as_bytes),
    );
    id.field(
        b"object_format",
        match snapshot.provenance().object_format() {
            GitObjectFormat::Sha1 => b"sha1",
            GitObjectFormat::Sha256 => b"sha256",
        },
    );
}

fn add_repository_source(id: &mut EvidenceIdBuilder, source: &RepositorySource) {
    if let Some(repository_id) = source.local_repository_id() {
        id.field(b"source_kind", b"local");
        id.field(b"repository_id", repository_id.as_str().as_bytes());
    } else if let Some((provider, namespace, repository)) = source.hosted_locator() {
        id.field(b"source_kind", b"hosted");
        id.field(b"provider", provider.as_bytes());
        id.field(b"namespace", namespace.as_bytes());
        id.field(b"repository", repository.as_bytes());
    }
}

pub(crate) fn add_raw_payload(id: &mut EvidenceIdBuilder, payload: &RawEvidencePayload) {
    match payload.data() {
        RawEvidencePayloadData::RepositorySnapshot => {
            id.field(b"payload", b"repository_snapshot");
        }
        RawEvidencePayloadData::TrackedFile(payload) => {
            id.field(b"payload", b"tracked_file");
            id.field(b"mode", entry_mode_code(payload.mode).as_bytes());
            id.field(
                b"object_kind",
                object_kind_code(payload.object_kind).as_bytes(),
            );
            id.optional_field(
                b"size_bytes",
                payload
                    .size_bytes
                    .as_ref()
                    .map(|value| value.to_string())
                    .as_deref()
                    .map(str::as_bytes),
            );
            id.optional_field(
                b"content_hash",
                payload
                    .content_hash
                    .as_ref()
                    .map(ContentHash::as_str)
                    .map(str::as_bytes),
            );
            id.optional_field(
                b"issue",
                payload.issue.map(raw_issue_code).map(str::as_bytes),
            );
        }
        RawEvidencePayloadData::HistoryScope(payload) => {
            id.field(b"payload", b"history_scope");
            id.optional_field(
                b"reachable_commits",
                payload
                    .reachable_commits
                    .as_ref()
                    .map(|value| value.to_string())
                    .as_deref()
                    .map(str::as_bytes),
            );
            id.optional_field(
                b"truncated",
                payload.truncated.map(|value| {
                    if value {
                        b"true".as_slice()
                    } else {
                        b"false".as_slice()
                    }
                }),
            );
            id.optional_field(
                b"issue",
                payload.issue.map(raw_issue_code).map(str::as_bytes),
            );
        }
        RawEvidencePayloadData::ParentDelta(payload) => {
            id.field(b"payload", b"parent_delta");
            id.optional_field(
                b"changed_entries",
                payload
                    .changed_entries
                    .as_ref()
                    .map(|value| value.to_string())
                    .as_deref()
                    .map(str::as_bytes),
            );
            id.optional_field(
                b"renames",
                payload
                    .renames
                    .as_ref()
                    .map(|value| value.to_string())
                    .as_deref()
                    .map(str::as_bytes),
            );
            id.optional_field(
                b"issue",
                payload.issue.map(raw_issue_code).map(str::as_bytes),
            );
        }
    }
}

pub(crate) fn add_classification_payload(
    id: &mut EvidenceIdBuilder,
    payload: &ClassificationPayload,
) {
    id.field(
        b"category",
        classification_category_code(payload.category).as_bytes(),
    );
    for tag in &payload.tags {
        id.field(b"tag", classification_tag_code(*tag).as_bytes());
    }
    id.field(b"rule_id", payload.rule_id.as_bytes());
    id.field(
        b"confidence_basis_points",
        payload.confidence_basis_points.to_string().as_bytes(),
    );
    for evidence in &payload.evidence {
        id.field(
            b"evidence_kind",
            classification_evidence_kind_code(evidence.kind()).as_bytes(),
        );
        id.field(b"evidence_rule_id", evidence.rule_id().as_bytes());
        id.optional_field(
            b"attribute_name",
            evidence.attribute_name().map(str::as_bytes),
        );
        id.optional_field(
            b"attribute_value",
            evidence.attribute_value().map(|value| {
                if value {
                    b"true".as_slice()
                } else {
                    b"false".as_slice()
                }
            }),
        );
    }
}

pub(crate) fn portable_path_bytes(path: &PortableRepositoryPath) -> Option<Vec<u8>> {
    match path.encoding {
        PortablePathEncoding::Utf8 => Some(path.value.as_bytes().to_vec()),
        PortablePathEncoding::GitPathHex => decode_hex(&path.value),
    }
}
