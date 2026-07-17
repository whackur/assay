//! Consent and section-status model for private-source features.
//!
//! Private-source AI evaluation and public-competitor discovery default to
//! `disabled` with reason `user_consent_required`. The dashboard never forks
//! its shape: every section reports a status, a reason, and an allowed next
//! action. Only explicit informed consent that acknowledges the provider and
//! the transmitted-evidence scope may enable an external provider.

use serde::Serialize;

/// A private-source feature that transmits derived evidence off the machine.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivateFeature {
    /// AI evaluation of private source via an external provider.
    AiEvaluation,
    /// Public competitor discovery derived from private source.
    CompetitorDiscovery,
}

/// The rendered status of a report section.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SectionState {
    Complete,
    Partial,
    Pending,
    Disabled,
    Unavailable,
}

/// A machine-stable reason code for a section state.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SectionReason {
    UserConsentRequired,
    ProviderUnavailable,
    AwaitingCompletion,
    Consented,
}

/// The single next action a section offers, if any.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NextAction {
    GrantConsent,
    AwaitCompletion,
    ContactOperator,
    None,
}

/// The report-level external-transmission posture, mirroring the web contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalTransmission {
    NotRequested,
    Prohibited,
    ConsentRequired,
    Consented,
}

/// The transmission *surface* one consent acknowledgement covers (ADR 0012).
///
/// `BundleOnly` acknowledges that only bounded, derived evidence facts reach
/// the external provider (the API-key family). `WorktreeSnapshot` acknowledges
/// by name that an agentic provider may read and transmit any file of the
/// analyzed revision — a strictly broader acknowledgement that is required
/// even for a public-only repository, because the provider vendor receives
/// the content either way. The report contract presents the two surfaces as
/// distinct acknowledgements; one never implies the other.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TransmissionSurface {
    BundleOnly,
    WorktreeSnapshot,
}

/// A rendered section: status plus reason plus the single allowed next action,
/// plus the acknowledged transmission surface once consent exists.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct SectionReport {
    pub state: SectionState,
    pub reason: SectionReason,
    pub next_action: NextAction,
    /// The surface the governing consent acknowledged; absent while the
    /// section is still awaiting consent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acknowledged_surface: Option<TransmissionSurface>,
}

/// An identifier for the external provider a user consented to.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalProvider(String);

impl ExternalProvider {
    /// Names an external provider (for example `openai_api`).
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the provider identifier.
    pub fn id(&self) -> &str {
        &self.0
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_sections_are_disabled_pending_consent() {
        let state = ConsentState::default();
        for feature in [
            PrivateFeature::AiEvaluation,
            PrivateFeature::CompetitorDiscovery,
        ] {
            let section = state.section(feature);
            assert_eq!(section.state, SectionState::Disabled);
            assert_eq!(section.reason, SectionReason::UserConsentRequired);
            assert_eq!(section.next_action, NextAction::GrantConsent);
        }
        assert_eq!(
            state.external_transmission(),
            ExternalTransmission::ConsentRequired
        );
    }

    #[test]
    fn consent_moves_section_off_disabled() {
        let grant = ConsentGrant::acknowledge(
            ExternalProvider::new("openai_api"),
            "classified evidence facts only",
        );
        let state = ConsentState::default().granting(PrivateFeature::AiEvaluation, grant);
        let section = state.section(PrivateFeature::AiEvaluation);
        assert_ne!(section.state, SectionState::Disabled);
        assert_eq!(
            state.external_transmission(),
            ExternalTransmission::Consented
        );
        assert_eq!(
            state.section(PrivateFeature::CompetitorDiscovery).state,
            SectionState::Disabled
        );
    }

    #[test]
    fn bundle_consent_acknowledges_only_the_bundle_surface() {
        let grant = ConsentGrant::acknowledge(
            ExternalProvider::new("openai_api"),
            "classified evidence facts only",
        );
        assert_eq!(
            grant.acknowledged_surface(),
            TransmissionSurface::BundleOnly
        );
        let state = ConsentState::default().granting(PrivateFeature::AiEvaluation, grant);
        let section = state.section(PrivateFeature::AiEvaluation);
        assert_eq!(
            section.acknowledged_surface,
            Some(TransmissionSurface::BundleOnly)
        );
    }

    #[test]
    fn agentic_consent_acknowledges_the_worktree_snapshot_surface_by_name() {
        let grant = ConsentGrant::acknowledge_worktree_snapshot(
            ExternalProvider::new("codex_cli"),
            "this agent may read and transmit any file of the analyzed revision",
        );
        assert_eq!(
            grant.acknowledged_surface(),
            TransmissionSurface::WorktreeSnapshot
        );
        let state = ConsentState::default().granting(PrivateFeature::AiEvaluation, grant);
        let section = state.section(PrivateFeature::AiEvaluation);
        assert_eq!(
            section.acknowledged_surface,
            Some(TransmissionSurface::WorktreeSnapshot)
        );
        // The two surfaces stay distinct acknowledgements in serialized form.
        let value = serde_json::to_value(section).unwrap();
        assert_eq!(value["acknowledged_surface"], "worktree_snapshot");
    }

    #[test]
    fn sections_without_consent_serialize_without_a_surface_key() {
        let section = ConsentState::default().section(PrivateFeature::AiEvaluation);
        let value = serde_json::to_value(section).unwrap();
        assert!(value.get("acknowledged_surface").is_none());
    }
}
