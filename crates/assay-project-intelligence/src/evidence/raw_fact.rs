use std::fmt;

use assay_domain::{ContentHash, EvidenceId, EvidenceStatus};

use crate::evidence::payload::RawEvidencePayload;
use crate::evidence::payload::RawEvidencePayloadData;
use crate::evidence::source::EvidenceSourceRecord;
use crate::evidence::types::RawEvidenceKind;

/// One immutable raw fact, kept separate from derived classification.
#[derive(Clone, Eq, PartialEq)]
pub struct RawEvidenceFact {
    pub(crate) id: EvidenceId,
    pub(crate) kind: RawEvidenceKind,
    pub(crate) status: EvidenceStatus,
    pub(crate) source: EvidenceSourceRecord,
    pub(crate) payload: RawEvidencePayload,
}

impl RawEvidenceFact {
    /// Returns the versioned, content-derived evidence identifier.
    pub const fn id(&self) -> &EvidenceId {
        &self.id
    }

    /// Returns the raw fact kind used for ID domain separation.
    pub const fn kind(&self) -> RawEvidenceKind {
        self.kind
    }

    /// Returns exact source availability without numeric substitution.
    pub const fn status(&self) -> EvidenceStatus {
        self.status
    }

    /// Returns portable immutable citation data.
    pub const fn source(&self) -> &EvidenceSourceRecord {
        &self.source
    }

    /// Returns the typed raw payload.
    pub const fn payload(&self) -> &RawEvidencePayload {
        &self.payload
    }

    /// Returns a complete content digest only when one was collected.
    pub const fn content_hash(&self) -> Option<&ContentHash> {
        match &self.payload.data() {
            RawEvidencePayloadData::TrackedFile(payload) => payload.content_hash.as_ref(),
            _ => None,
        }
    }
}

impl fmt::Debug for RawEvidenceFact {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RawEvidenceFact")
            .field("id", &self.id)
            .field("kind", &self.kind)
            .field("status", &self.status)
            .field("source", &self.source)
            .field("payload", &self.payload)
            .finish()
    }
}
