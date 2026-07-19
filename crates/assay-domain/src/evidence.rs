use serde::{Deserialize, Deserializer, Serialize, de};

use crate::error::DomainValueError;
use crate::hashes::ContentHash;
use crate::identifiers::{EvidenceId, RevisionId};
use crate::status::{EvidenceSourceKind, EvidenceStatus};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EvidenceSourceData {
    id: EvidenceId,
    kind: EvidenceSourceKind,
    status: EvidenceStatus,
    revision: Option<RevisionId>,
    content_hash: Option<ContentHash>,
}

/// Provenance and availability for one stable evidence identifier.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EvidenceSource {
    id: EvidenceId,
    kind: EvidenceSourceKind,
    status: EvidenceStatus,
    revision: Option<RevisionId>,
    content_hash: Option<ContentHash>,
}

impl EvidenceSource {
    /// Creates evidence pinned to an immutable source revision.
    pub const fn at_revision(
        id: EvidenceId,
        kind: EvidenceSourceKind,
        status: EvidenceStatus,
        revision: RevisionId,
    ) -> Self {
        Self {
            id,
            kind,
            status,
            revision: Some(revision),
            content_hash: None,
        }
    }

    /// Creates content evidence pinned to both a revision and SHA-256 digest.
    pub const fn at_content(
        id: EvidenceId,
        kind: EvidenceSourceKind,
        status: EvidenceStatus,
        revision: RevisionId,
        content_hash: ContentHash,
    ) -> Self {
        Self {
            id,
            kind,
            status,
            revision: Some(revision),
            content_hash: Some(content_hash),
        }
    }

    /// Creates an explicit unresolved source for a non-usable evidence state.
    pub fn unresolved(
        id: EvidenceId,
        kind: EvidenceSourceKind,
        status: EvidenceStatus,
    ) -> Result<Self, DomainValueError> {
        Self::validate_provenance(status, None, None)?;
        Ok(Self {
            id,
            kind,
            status,
            revision: None,
            content_hash: None,
        })
    }

    /// Returns the stable evidence identifier.
    pub const fn id(&self) -> &EvidenceId {
        &self.id
    }

    /// Returns the stable provenance category.
    pub const fn kind(&self) -> EvidenceSourceKind {
        self.kind
    }

    /// Returns availability without inferring the overall analysis state.
    pub const fn status(&self) -> EvidenceStatus {
        self.status
    }

    /// Returns the immutable source revision when known.
    pub const fn revision(&self) -> Option<&RevisionId> {
        self.revision.as_ref()
    }

    /// Returns the content digest when evidence was content-addressed.
    pub const fn content_hash(&self) -> Option<&ContentHash> {
        self.content_hash.as_ref()
    }

    fn validate_provenance(
        status: EvidenceStatus,
        revision: Option<&RevisionId>,
        content_hash: Option<&ContentHash>,
    ) -> Result<(), DomainValueError> {
        if matches!(status, EvidenceStatus::Complete | EvidenceStatus::Partial)
            && revision.is_none()
            && content_hash.is_none()
        {
            return Err(DomainValueError::new(
                "evidence_source",
                "complete or partial evidence requires immutable provenance",
            ));
        }
        Ok(())
    }
}

impl TryFrom<EvidenceSourceData> for EvidenceSource {
    type Error = DomainValueError;

    fn try_from(value: EvidenceSourceData) -> Result<Self, Self::Error> {
        Self::validate_provenance(
            value.status,
            value.revision.as_ref(),
            value.content_hash.as_ref(),
        )?;
        Ok(Self {
            id: value.id,
            kind: value.kind,
            status: value.status,
            revision: value.revision,
            content_hash: value.content_hash,
        })
    }
}

impl<'de> Deserialize<'de> for EvidenceSource {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_from(EvidenceSourceData::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}
