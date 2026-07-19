//! Public consent and section-status types.
//!
//! Private-source features default to disabled with reason
//! `user_consent_required`. The dashboard never forks its shape: every section
//! reports a state, a reason, and a single allowed next action, plus the
//! acknowledged transmission surface once consent exists.

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
