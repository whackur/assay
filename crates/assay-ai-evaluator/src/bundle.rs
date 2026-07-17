use std::collections::BTreeSet;

use assay_domain::EvidenceId;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{EvaluationError, EvaluationErrorKind};

const EVIDENCE_BUNDLE_DOMAIN: &[u8] = b"assay.ai-evaluator.evidence-bundle.v1";
const MAX_EVIDENCE_STATEMENT_BYTES: usize = 1_000;

/// Bounded evidence category supplied to a qualitative evaluator.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    RepositoryFact,
    DocumentationClaim,
    ImplementationFact,
    Test,
    ReportedCi,
    ReleaseFact,
    RepositoryConfiguration,
    ComparisonFact,
}

impl EvidenceKind {
    const fn code(self) -> &'static str {
        match self {
            Self::RepositoryFact => "repository_fact",
            Self::DocumentationClaim => "documentation_claim",
            Self::ImplementationFact => "implementation_fact",
            Self::Test => "test",
            Self::ReportedCi => "reported_ci",
            Self::ReleaseFact => "release_fact",
            Self::RepositoryConfiguration => "repository_configuration",
            Self::ComparisonFact => "comparison_fact",
        }
    }
}

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

/// One citable, bounded statement derived from deterministic evidence.
#[derive(Clone, Eq, PartialEq)]
pub struct EvidenceDescriptor {
    id: EvidenceId,
    kind: EvidenceKind,
    statement: String,
}

impl EvidenceDescriptor {
    /// Creates a provider-safe descriptor without raw source, diffs, secrets,
    /// host paths, prompt instructions, or person-level evaluation language.
    pub fn new(
        id: EvidenceId,
        kind: EvidenceKind,
        statement: &str,
    ) -> Result<Self, EvaluationError> {
        validate_untrusted_text(statement, TextPolicy::Evidence)?;
        Ok(Self {
            id,
            kind,
            statement: statement.to_owned(),
        })
    }

    /// Returns the stable evidence citation identifier.
    pub const fn id(&self) -> &EvidenceId {
        &self.id
    }

    /// Returns the bounded fact category.
    pub const fn kind(&self) -> EvidenceKind {
        self.kind
    }

    /// Returns the reviewed bounded statement, not source or raw diff text.
    pub fn statement(&self) -> &str {
        &self.statement
    }
}

impl std::fmt::Debug for EvidenceDescriptor {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EvidenceDescriptor")
            .field("id", &self.id)
            .field("kind", &self.kind)
            .field("statement", &"<bounded-evidence>")
            .finish()
    }
}

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
        items.sort_by(|left, right| left.id.cmp(&right.id));
        if items.windows(2).any(|pair| pair[0].id == pair[1].id) {
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
        self.items.binary_search_by(|item| item.id.cmp(id)).is_ok()
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

#[derive(Clone, Copy)]
pub(crate) enum TextPolicy {
    Evidence,
    ProviderRationale,
}

pub(crate) fn validate_untrusted_text(
    value: &str,
    _policy: TextPolicy,
) -> Result<(), EvaluationError> {
    if value.is_empty()
        || value.len() > MAX_EVIDENCE_STATEMENT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(EvaluationError::new(
            EvaluationErrorKind::EvidenceTextInvalid,
        ));
    }
    let lower = value.to_ascii_lowercase();
    const INJECTION_PHRASES: &[&str] = &[
        "ignore previous instruction",
        "ignore all previous",
        "jailbreak",
    ];
    // Role labels signal injection only when they open the statement, not mid-prose.
    const INJECTION_PREFIXES: &[&str] = &[
        "system message",
        "developer message",
        "<system",
        "assistant:",
        "system:",
        "developer:",
    ];
    let opening = lower.trim_start_matches(|character: char| {
        character.is_whitespace() || matches!(character, '>' | '#' | '-' | '*' | '"' | '\'' | '`')
    });
    if INJECTION_PHRASES
        .iter()
        .any(|marker| lower.contains(marker))
        || INJECTION_PREFIXES
            .iter()
            .any(|marker| opening.starts_with(marker))
    {
        return Err(EvaluationError::new(EvaluationErrorKind::PromptInjection));
    }
    const SENSITIVE_MARKERS: &[&str] = &[
        "authorization: bearer",
        "api_key=",
        "api-key=",
        "access_token=",
        "refresh_token=",
        "password=",
        "private key-----",
        "-----begin private key",
        "chatgpt oauth",
        "auth.json",
    ];
    if SENSITIVE_MARKERS
        .iter()
        .any(|marker| lower.contains(marker))
        || lower.split_whitespace().any(|part| part.starts_with("sk-"))
    {
        return Err(EvaluationError::new(EvaluationErrorKind::SensitiveContent));
    }
    if lower.lines().any(|line| {
        line.starts_with("diff --git ")
            || line.starts_with("@@ ")
            || line.starts_with("+++ ")
            || line.starts_with("--- ")
    }) {
        return Err(EvaluationError::new(EvaluationErrorKind::RawDiff));
    }
    if contains_absolute_path(value) {
        return Err(EvaluationError::new(EvaluationErrorKind::AbsolutePath));
    }
    const PERSON_MARKERS: &[&str] = &[
        "developer productivity",
        "contributor performance",
        "employee performance",
        "compensation decision",
        "promotion decision",
        "hiring recommendation",
        "individual contributor score",
    ];
    if PERSON_MARKERS.iter().any(|marker| lower.contains(marker)) {
        return Err(EvaluationError::new(
            EvaluationErrorKind::PersonDomainMixing,
        ));
    }
    Ok(())
}

fn contains_absolute_path(value: &str) -> bool {
    value.split_whitespace().any(|token| {
        let token = token.trim_matches(|character: char| {
            matches!(
                character,
                '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';' | '"' | '\''
            )
        });
        (token.starts_with('/') && token[1..].contains('/'))
            || (token.len() >= 3
                && token.as_bytes()[0].is_ascii_alphabetic()
                && token.as_bytes()[1] == b':'
                && matches!(token.as_bytes()[2], b'/' | b'\\'))
    })
}

fn bundle_hash(
    scope: EvidenceScope,
    transmission: ExternalTransmission,
    items: &[EvidenceDescriptor],
) -> String {
    let mut hash = Sha256::new();
    update_length_prefixed(&mut hash, EVIDENCE_BUNDLE_DOMAIN);
    update_length_prefixed(&mut hash, privacy_code(scope));
    update_length_prefixed(&mut hash, transmission_code(transmission));
    for item in items {
        update_length_prefixed(&mut hash, item.id.as_str().as_bytes());
        update_length_prefixed(&mut hash, item.kind.code().as_bytes());
        update_length_prefixed(&mut hash, item.statement.as_bytes());
    }
    format!("sha256:{:x}", hash.finalize())
}

fn update_length_prefixed(hash: &mut Sha256, value: &[u8]) {
    hash.update((value.len() as u64).to_be_bytes());
    hash.update(value);
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

pub(crate) fn id_set(bundle: &EvidenceBundle) -> BTreeSet<&str> {
    bundle.items.iter().map(|item| item.id.as_str()).collect()
}
