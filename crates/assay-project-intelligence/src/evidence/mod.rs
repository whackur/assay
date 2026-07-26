//! Deterministic project-level evidence assembly. Combines immutable Git snapshot facts with versioned file classification facts. No I/O. The typed manifest is not a serialized public contract; CLI mapping to `schemas/` is a later boundary.
//! Measures which facts were collected and how files were classified — not whether a project builds, works, is safe, original, or valuable. Never a project score or person-level signal. Missing/partial/unsupported evidence stays explicit, never numeric zero.

mod assembly;
mod classification_record;
mod codes;
mod error;
mod hex;
mod id;
mod manifest;
mod mapping;
mod payload;
mod raw_fact;
mod source;
mod types;

#[cfg(test)]
mod tests;

pub use assembly::assemble_project_evidence;
pub use classification_record::{
    ClassificationEvidenceFact, ClassificationEvidenceRecord, ClassifiedSnapshotFile,
};
pub use error::{EvidenceAssemblyError, EvidenceAssemblyErrorKind};
pub use manifest::ProjectEvidenceManifest;
pub use payload::{
    HistoryScopeEvidence, ParentDeltaEvidence, RawEvidencePayload, TrackedFileEvidence,
};
pub use raw_fact::RawEvidenceFact;
pub use source::{EvidenceSourceRecord, GitEvidenceProvenance};
pub use types::{
    ClassificationAvailabilityReason, ClassificationCategoryRecord,
    ClassificationEvidenceKindRecord, ClassificationTagRecord, GitObjectFormatRecord,
    PortablePathEncoding, PortableRepositoryPath, RawEvidenceIssue, RawEvidenceKind,
};
