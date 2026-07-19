use serde::{Deserialize, Serialize};

/// Privacy scope attached to the exact evidence bundle.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceScope {
    PublicOnly,
    PrivateLocal,
}

/// Whether evidence may cross the local evaluator boundary.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalTransmission {
    NotUsed,
    PublicOnly,
    ConsentedPrivate,
}

/// The transmission *surface* a consent acknowledgement covers (ADR 0012).
///
/// `BundleOnly` is the API-key family surface: only the bounded evidence
/// bundle can reach an external provider. `WorktreeSnapshot` is the agentic
/// family surface: the agent may read and transmit any file of the analyzed
/// revision, so consent must acknowledge this broader surface by name even
/// for public repositories.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TransmissionSurface {
    BundleOnly,
    WorktreeSnapshot,
}

pub(crate) const fn privacy_code(scope: EvidenceScope) -> &'static [u8] {
    match scope {
        EvidenceScope::PublicOnly => b"public_only",
        EvidenceScope::PrivateLocal => b"private_local",
    }
}

pub(crate) const fn transmission_code(transmission: ExternalTransmission) -> &'static [u8] {
    match transmission {
        ExternalTransmission::NotUsed => b"not_used",
        ExternalTransmission::PublicOnly => b"public_only",
        ExternalTransmission::ConsentedPrivate => b"consented_private",
    }
}
