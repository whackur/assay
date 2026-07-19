//! Consent grants and the aggregate consent posture.
//!
//! A [`ConsentGrant`] can only be constructed by naming the provider, the
//! transmission surface, and a transmitted-evidence description, so a grant
//! cannot exist without an acknowledged transmission. [`ConsentState`] holds
//! the grants for all private-source features and renders their sections.

use super::types::{
    ExternalProvider, ExternalTransmission, NextAction, PrivateFeature, SectionReason,
    SectionReport, SectionState, TransmissionSurface,
};

/// Explicit informed consent for one feature. Constructing a grant requires
/// naming the provider, the transmission surface, and a transmitted-evidence
/// description, so a grant cannot exist without an acknowledged transmission.
/// The formalized [`TransmissionSurface`] carries the machine-checked scope;
/// the free-text description remains for human display only.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConsentGrant {
    provider: ExternalProvider,
    surface: TransmissionSurface,
    evidence_scope: String,
}

impl ConsentGrant {
    /// Records acknowledged bundle-facts consent for `provider`: only the
    /// bounded evidence bundle may reach the provider (the API-key family).
    pub fn acknowledge(provider: ExternalProvider, evidence_scope: impl Into<String>) -> Self {
        Self {
            provider,
            surface: TransmissionSurface::BundleOnly,
            evidence_scope: evidence_scope.into(),
        }
    }

    /// Records acknowledged whole-snapshot consent for an agentic `provider`:
    /// the agent may read and transmit any file of the analyzed revision, not
    /// merely the bundle facts. Required even for public-only repositories.
    pub fn acknowledge_worktree_snapshot(
        provider: ExternalProvider,
        evidence_scope: impl Into<String>,
    ) -> Self {
        Self {
            provider,
            surface: TransmissionSurface::WorktreeSnapshot,
            evidence_scope: evidence_scope.into(),
        }
    }

    /// Returns the acknowledged provider.
    pub fn provider(&self) -> &ExternalProvider {
        &self.provider
    }

    /// Returns the transmission surface this grant acknowledged by name.
    pub const fn acknowledged_surface(&self) -> TransmissionSurface {
        self.surface
    }

    /// Returns the acknowledged transmitted-evidence description.
    pub fn evidence_scope(&self) -> &str {
        &self.evidence_scope
    }
}

/// The consent posture for all private-source features. Defaults to no grants,
/// which renders every private feature disabled and pending consent.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ConsentState {
    ai_evaluation: Option<ConsentGrant>,
    competitor_discovery: Option<ConsentGrant>,
}

impl ConsentState {
    /// Grants consent for a feature, returning the updated state.
    pub fn granting(mut self, feature: PrivateFeature, grant: ConsentGrant) -> Self {
        match feature {
            PrivateFeature::AiEvaluation => self.ai_evaluation = Some(grant),
            PrivateFeature::CompetitorDiscovery => self.competitor_discovery = Some(grant),
        }
        self
    }

    fn grant(&self, feature: PrivateFeature) -> Option<&ConsentGrant> {
        match feature {
            PrivateFeature::AiEvaluation => self.ai_evaluation.as_ref(),
            PrivateFeature::CompetitorDiscovery => self.competitor_discovery.as_ref(),
        }
    }

    /// Renders a feature section. Without consent the section is disabled and
    /// offers only `grant_consent`. With consent but no wired provider it is
    /// `unavailable` because no external provider runs in the local slice; the
    /// section then reports the exact surface the consent acknowledged, so
    /// bundle-facts and full-snapshot consent stay distinct acknowledgements.
    pub fn section(&self, feature: PrivateFeature) -> SectionReport {
        match self.grant(feature) {
            None => SectionReport {
                state: SectionState::Disabled,
                reason: SectionReason::UserConsentRequired,
                next_action: NextAction::GrantConsent,
                acknowledged_surface: None,
            },
            Some(grant) => SectionReport {
                state: SectionState::Unavailable,
                reason: SectionReason::ProviderUnavailable,
                next_action: NextAction::ContactOperator,
                acknowledged_surface: Some(grant.acknowledged_surface()),
            },
        }
    }

    /// Reports whether any external transmission has been consented.
    pub fn external_transmission(&self) -> ExternalTransmission {
        if self.ai_evaluation.is_some() || self.competitor_discovery.is_some() {
            ExternalTransmission::Consented
        } else {
            ExternalTransmission::ConsentRequired
        }
    }
}
