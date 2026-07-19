//! Consent and section-status model for private-source features.
//!
//! Private-source AI evaluation and public-competitor discovery default to
//! `disabled` with reason `user_consent_required`. The dashboard never forks
//! its shape: every section reports a status, a reason, and an allowed next
//! action. Only explicit informed consent that acknowledges the provider and
//! the transmitted-evidence scope may enable an external provider.
//!
//! The module is split by responsibility: [`types`] owns the public enums,
//! the rendered [`SectionReport`], and the [`ExternalProvider`] identifier;
//! [`state`] owns [`ConsentGrant`] and the aggregate [`ConsentState`].

mod state;
mod types;

pub use state::{ConsentGrant, ConsentState};
pub use types::{
    ExternalProvider, ExternalTransmission, NextAction, PrivateFeature, SectionReason,
    SectionReport, SectionState, TransmissionSurface,
};

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
