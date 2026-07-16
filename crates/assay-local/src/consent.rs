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

/// A rendered section: status plus reason plus the single allowed next action.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct SectionReport {
    pub state: SectionState,
    pub reason: SectionReason,
    pub next_action: NextAction,
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
/// naming the provider and describing the transmitted-evidence scope, so a
/// grant cannot exist without an acknowledged transmission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConsentGrant {
    provider: ExternalProvider,
    evidence_scope: String,
}

impl ConsentGrant {
    /// Records acknowledged consent for `provider` and `evidence_scope`.
    pub fn acknowledge(provider: ExternalProvider, evidence_scope: impl Into<String>) -> Self {
        Self {
            provider,
            evidence_scope: evidence_scope.into(),
        }
    }

    /// Returns the acknowledged provider.
    pub fn provider(&self) -> &ExternalProvider {
        &self.provider
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
    /// `unavailable` because no external provider runs in the local slice.
    pub fn section(&self, feature: PrivateFeature) -> SectionReport {
        match self.grant(feature) {
            None => SectionReport {
                state: SectionState::Disabled,
                reason: SectionReason::UserConsentRequired,
                next_action: NextAction::GrantConsent,
            },
            Some(_) => SectionReport {
                state: SectionState::Unavailable,
                reason: SectionReason::ProviderUnavailable,
                next_action: NextAction::ContactOperator,
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
}
