use crate::{
    EvaluationError, EvaluationErrorKind, EvidenceBundle, EvidenceScope, ExternalTransmission,
    TransmissionSurface,
};

use super::types::ProviderExecutionBoundary;

/// Rejects any provider boundary that would move evidence past its consent.
///
/// An external provider transmitting the `WorktreeSnapshot` surface requires a
/// consent that acknowledged that surface by name, even for a public-only
/// repository, because the provider vendor receives the whole analyzed tree
/// rather than only the bounded bundle facts (ADR 0012).
pub(crate) fn enforce_transmission_boundary(
    boundary: ProviderExecutionBoundary,
    surface: TransmissionSurface,
    bundle: &EvidenceBundle,
) -> Result<(), EvaluationError> {
    if boundary == ProviderExecutionBoundary::External
        && surface == TransmissionSurface::WorktreeSnapshot
        && bundle.acknowledged_surface() != TransmissionSurface::WorktreeSnapshot
    {
        return Err(EvaluationError::new(EvaluationErrorKind::PrivacyMismatch));
    }
    match (boundary, bundle.transmission()) {
        (ProviderExecutionBoundary::Local, ExternalTransmission::NotUsed) => Ok(()),
        (ProviderExecutionBoundary::External, ExternalTransmission::PublicOnly)
            if bundle.scope() == EvidenceScope::PublicOnly =>
        {
            Ok(())
        }
        (ProviderExecutionBoundary::External, ExternalTransmission::ConsentedPrivate)
            if bundle.scope() == EvidenceScope::PrivateLocal =>
        {
            Ok(())
        }
        _ => Err(EvaluationError::new(EvaluationErrorKind::PrivacyMismatch)),
    }
}
