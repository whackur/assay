//! Deterministic project-level evidence assembly for Assay.
//!
//! This module combines immutable Git snapshot facts with versioned file
//! classification facts. It performs no filesystem, process, database, HTTP,
//! GitHub, or model-provider I/O. The resulting typed manifest is intentionally
//! not a serialized public machine contract; CLI mapping to the authoritative
//! schemas under `schemas/` is a later boundary.
//!
//! The manifest measures which repository facts were collected and how tracked
//! files were classified. It cannot establish that a project builds, works, is
//! safe, is original, or is valuable. It must not be interpreted as a project
//! score, a person-level observation, or evidence about contributor intent or
//! productivity. Missing, partial, and unsupported evidence remain explicit
//! availability states rather than numeric zeroes.

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
