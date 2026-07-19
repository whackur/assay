mod descriptor;
mod hash;
mod kind;
mod scope;
mod text;

pub use descriptor::EvidenceDescriptor;
pub use kind::EvidenceKind;
pub use scope::{EvidenceScope, ExternalTransmission, TransmissionSurface};
pub(crate) use text::{TextPolicy, validate_untrusted_text};

use assay_domain::EvidenceId;

use crate::{EvaluationError, EvaluationErrorKind};

use self::hash::bundle_hash;

/// Canonical, content-addressed evidence presented to one provider call.
#[derive(Clone, Eq, PartialEq)]
pub struct EvidenceBundle {
    scope: EvidenceScope,
    transmission: ExternalTransmission,
    acknowledged_surface: TransmissionSurface,
    items: Vec<EvidenceDescriptor>,
    content_hash: String,
}

impl EvidenceBundle {
    /// Validates privacy and canonicalizes items by evidence ID. The governing
    /// consent acknowledges only the bounded bundle surface, so no provider
    /// that transmits a whole worktree snapshot can pass boundary enforcement.
    pub fn new(
        scope: EvidenceScope,
        transmission: ExternalTransmission,
        items: Vec<EvidenceDescriptor>,
    ) -> Result<Self, EvaluationError> {
        Self::with_acknowledged_surface(scope, transmission, TransmissionSurface::BundleOnly, items)
    }

    /// Validates privacy and canonicalizes items, recording the transmission
    /// surface the governing consent acknowledged. `WorktreeSnapshot` states
    /// that the provider may read and transmit any file of the analyzed
    /// revision, not merely the bundle facts.
    ///
    /// The surface gates transmission before any provider is called; it is not
    /// part of the evidence content identity a judgment binds to, so it does
    /// not enter the bundle content hash.
    pub fn with_acknowledged_surface(
        scope: EvidenceScope,
        transmission: ExternalTransmission,
        acknowledged_surface: TransmissionSurface,
        mut items: Vec<EvidenceDescriptor>,
    ) -> Result<Self, EvaluationError> {
        if items.is_empty() {
            return Err(EvaluationError::new(
                EvaluationErrorKind::EmptyEvidenceBundle,
            ));
        }
        if scope == EvidenceScope::PrivateLocal && transmission == ExternalTransmission::PublicOnly
        {
            return Err(EvaluationError::new(EvaluationErrorKind::PrivacyMismatch));
        }
        if scope == EvidenceScope::PublicOnly
            && transmission == ExternalTransmission::ConsentedPrivate
        {
            return Err(EvaluationError::new(EvaluationErrorKind::PrivacyMismatch));
        }
        items.sort_by(|left, right| left.id().cmp(right.id()));
        if items.windows(2).any(|pair| pair[0].id() == pair[1].id()) {
            return Err(EvaluationError::new(EvaluationErrorKind::DuplicateEvidence));
        }
        let content_hash = bundle_hash(scope, transmission, &items);
        Ok(Self {
            scope,
            transmission,
            acknowledged_surface,
            items,
            content_hash,
        })
    }

    /// Returns the evidence privacy scope.
    pub const fn scope(&self) -> EvidenceScope {
        self.scope
    }

    /// Returns the external-transmission policy.
    pub const fn transmission(&self) -> ExternalTransmission {
        self.transmission
    }

    /// Returns the transmission surface the governing consent acknowledged.
    pub const fn acknowledged_surface(&self) -> TransmissionSurface {
        self.acknowledged_surface
    }

    /// Returns evidence in canonical identifier order.
    pub fn items(&self) -> &[EvidenceDescriptor] {
        &self.items
    }

    /// Returns the domain-separated content hash used to bind provider output.
    pub fn content_hash(&self) -> &str {
        &self.content_hash
    }

    pub(crate) fn contains(&self, id: &EvidenceId) -> bool {
        self.items
            .binary_search_by(|item| item.id().cmp(id))
            .is_ok()
    }
}

impl std::fmt::Debug for EvidenceBundle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EvidenceBundle")
            .field("scope", &self.scope)
            .field("transmission", &self.transmission)
            .field("acknowledged_surface", &self.acknowledged_surface)
            .field("item_count", &self.items.len())
            .field("content_hash", &self.content_hash)
            .finish()
    }
}

pub(crate) use self::hash::id_set;
