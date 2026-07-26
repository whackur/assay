//! Consent grants and the aggregate consent posture.
//!
//! A [`ConsentGrant`] requires naming provider, surface, and evidence scope,
//! so a grant cannot exist without an acknowledged transmission. [`ConsentState`]
//! holds grants for all private-source features and renders their sections.

use super::types::{
    ExternalProvider, ExternalTransmission, NextAction, PrivateFeature, SectionReason,
    SectionReport, SectionState, TransmissionSurface,
};

/// Explicit informed consent for one feature. Construction requires naming
/// provider, surface, and evidence scope, so no grant exists without an
/// acknowledged transmission. The free-text scope is human display only.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConsentGrant {
    provider: ExternalProvider,
    surface: TransmissionSurface,
    evidence_scope: String,
}

impl ConsentGrant {
    /// Acknowledged bundle-facts consent: only the bounded evidence bundle may
    /// reach the provider (the API-key family).
    pub fn acknowledge(provider: ExternalProvider, evidence_scope: impl Into<String>) -> Self {
        Self {
            provider,
            surface: TransmissionSurface::BundleOnly,
            evidence_scope: evidence_scope.into(),
        }
    }

    /// Acknowledged whole-snapshot consent: the agent may read and transmit any
    /// file of the analyzed revision. Required even for public-only repositories.
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

    pub fn provider(&self) -> &ExternalProvider {
        &self.provider
    }

    pub const fn acknowledged_surface(&self) -> TransmissionSurface {
        self.surface
    }

    pub fn evidence_scope(&self) -> &str {
        &self.evidence_scope
    }
}

/// Consent posture for all private-source features. Defaults to no grants,
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

    /// Without consent the section is disabled offering only `grant_consent`.
    /// With consent but no wired provider it is `unavailable` (no external
    /// provider runs in the local slice) and reports the exact acknowledged
    /// surface, keeping bundle-facts and full-snapshot consent distinct.
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

    pub fn external_transmission(&self) -> ExternalTransmission {
        if self.ai_evaluation.is_some() || self.competitor_discovery.is_some() {
            ExternalTransmission::Consented
        } else {
            ExternalTransmission::ConsentRequired
        }
    }
}