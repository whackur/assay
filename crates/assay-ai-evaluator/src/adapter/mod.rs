//! Evidence-manifest to evaluation-bundle adapter (ADP-001).
//!
//! The CLI produces a [`ProjectEvidenceManifest`] from
//! `assay-project-intelligence` and the AI evaluator consumes an
//! [`EvidenceBundle`]. This module bridges the two without exposing raw source,
//! diffs, host paths, or person-level language: every manifest fact becomes
//! one bounded [`EvidenceDescriptor`] whose statement is derived from the
//! fact's kind and availability, never from the underlying content.
//!
//! The adapter is deterministic and performs no I/O. Identical manifests yield
//! byte-identical bundles, so the downstream bundle hash is stable.

mod statement;

pub use statement::ManifestAdapterError;

use assay_project_intelligence::{ProjectEvidenceManifest, RawEvidenceKind};

use crate::{
    EvidenceBundle, EvidenceDescriptor, EvidenceKind, EvidenceScope, ExternalTransmission,
};

/// Privacy scope and transmission policy for one adapter call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterPrivacy {
    scope: EvidenceScope,
    transmission: ExternalTransmission,
}

impl AdapterPrivacy {
    /// Local private evidence with no external transmission. The default for
    /// `assay project analyze` running the deterministic evaluator without
    /// consent: evidence stays on the machine and no provider is constructed.
    pub const fn local_deterministic() -> Self {
        Self {
            scope: EvidenceScope::PrivateLocal,
            transmission: ExternalTransmission::NotUsed,
        }
    }

    /// Returns the privacy scope.
    pub const fn scope(self) -> EvidenceScope {
        self.scope
    }

    /// Returns the external-transmission policy.
    pub const fn transmission(self) -> ExternalTransmission {
        self.transmission
    }
}

/// Converts a [`ProjectEvidenceManifest`] into a provider-safe
/// [`EvidenceBundle`] without raw source, diffs, or host paths.
pub fn manifest_to_evidence_bundle(
    manifest: &ProjectEvidenceManifest,
    privacy: AdapterPrivacy,
) -> Result<EvidenceBundle, ManifestAdapterError> {
    let mut items = Vec::with_capacity(manifest.raw_facts().len());
    for fact in manifest.raw_facts() {
        let kind = map_raw_kind(fact.kind());
        let statement = statement::raw_statement(fact)?;
        items.push(EvidenceDescriptor::new(
            fact.id().clone(),
            kind,
            &statement,
        )?);
    }
    for fact in manifest.classification_facts() {
        let Some(category) = fact.category() else {
            continue;
        };
        let statement = statement::classification_statement(fact, category)?;
        let descriptor = EvidenceDescriptor::new(
            fact.id().clone(),
            EvidenceKind::RepositoryConfiguration,
            &statement,
        )?;
        items.push(descriptor);
    }
    if items.is_empty() {
        return Err(ManifestAdapterError::EmptyManifest);
    }
    EvidenceBundle::new(privacy.scope(), privacy.transmission(), items)
        .map_err(ManifestAdapterError::from_evaluation_error)
}

fn map_raw_kind(kind: RawEvidenceKind) -> EvidenceKind {
    match kind {
        RawEvidenceKind::RepositorySnapshot => EvidenceKind::RepositoryFact,
        RawEvidenceKind::TrackedFile => EvidenceKind::ImplementationFact,
        RawEvidenceKind::HistoryScope => EvidenceKind::RepositoryFact,
        RawEvidenceKind::ParentDelta => EvidenceKind::RepositoryFact,
    }
}
