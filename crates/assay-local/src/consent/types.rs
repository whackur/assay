//! Public consent and section-status types.
//!
//! Private-source features default to disabled with reason `user_consent_required`.
//! The dashboard never forks its shape: every section reports a state, a reason,
//! and a single allowed next action, plus the acknowledged transmission surface.

use serde::Serialize;

/// Private-source feature that transmits derived evidence off the machine.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivateFeature {
    AiEvaluation,
    CompetitorDiscovery,
}

/// Rendered status of a report section.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SectionState {
    Complete,
    Partial,
    Pending,
    Disabled,
    Unavailable,
}

/// Machine-stable reason code for a section state.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SectionReason {
    UserConsentRequired,
    ProviderUnavailable,
    AwaitingCompletion,
    Consented,
}

/// Single next action a section offers, if any.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NextAction {
    GrantConsent,
    AwaitCompletion,
    ContactOperator,
    None,
}

/// Report-level external-transmission posture, mirroring the web contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalTransmission {
    NotRequested,
    Prohibited,
    ConsentRequired,
    Consented,
}

/// Transmission surface one consent acknowledgement covers (ADR 0012).
/// `WorktreeSnapshot` is a strictly broader acknowledgement than `BundleOnly`
/// and is required even for a public-only repo; one never implies the other.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TransmissionSurface {
    BundleOnly,
    WorktreeSnapshot,
}

/// Rendered section: status, reason, single allowed next action, and the
/// acknowledged transmission surface once consent exists.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct SectionReport {
    pub state: SectionState,
    pub reason: SectionReason,
    pub next_action: NextAction,
    /// Absent while the section is still awaiting consent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acknowledged_surface: Option<TransmissionSurface>,
}

/// Identifier for the external provider a user consented to.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalProvider(String);

impl ExternalProvider {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn id(&self) -> &str {
        &self.0
    }
}
