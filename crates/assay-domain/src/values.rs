use std::{error::Error, fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

const SHA256_PREFIX: &str = "sha256:";
const SHA256_HEX_LENGTH: usize = 64;
const GIT_SHA1_LENGTH: usize = 40;
const GIT_SHA256_LENGTH: usize = 64;
const MAX_COMPONENT_LENGTH: usize = 100;
const MAX_CODE_LENGTH: usize = 64;

/// A validation error that never echoes the rejected value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DomainValueError {
    value_kind: &'static str,
    reason: &'static str,
}

impl DomainValueError {
    fn new(value_kind: &'static str, reason: &'static str) -> Self {
        Self { value_kind, reason }
    }

    /// Returns the stable name of the rejected value type.
    pub const fn value_kind(&self) -> &'static str {
        self.value_kind
    }

    /// Returns a non-sensitive validation reason.
    pub const fn reason(&self) -> &'static str {
        self.reason
    }
}

impl fmt::Display for DomainValueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid {}: {}", self.value_kind, self.reason)
    }
}

impl Error for DomainValueError {}

macro_rules! validated_string_value {
    ($name:ident, $kind:literal, $validator:ident) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(String);

        impl $name {
            /// Returns the canonical serialized value.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl FromStr for $name {
            type Err = DomainValueError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                $validator(value).map_err(|reason| DomainValueError::new($kind, reason))?;
                Ok(Self(value.to_owned()))
            }
        }

        impl TryFrom<String> for $name {
            type Error = DomainValueError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                $validator(&value).map_err(|reason| DomainValueError::new($kind, reason))?;
                Ok(Self(value))
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::try_from(value).map_err(de::Error::custom)
            }
        }
    };
}

fn validate_revision(value: &str) -> Result<(), &'static str> {
    if !matches!(value.len(), GIT_SHA1_LENGTH | GIT_SHA256_LENGTH) {
        return Err("expected a full 40- or 64-character object identifier");
    }
    if !is_lower_hex(value) {
        return Err("expected lowercase hexadecimal characters");
    }
    if value.bytes().all(|byte| byte == b'0') {
        return Err("the Git null object identifier is not an immutable revision");
    }
    Ok(())
}

fn validate_sha256(value: &str) -> Result<(), &'static str> {
    let Some(digest) = value.strip_prefix(SHA256_PREFIX) else {
        return Err("expected a sha256-prefixed digest");
    };
    if digest.len() != SHA256_HEX_LENGTH {
        return Err("expected a 64-character SHA-256 digest");
    }
    if !is_lower_hex(digest) {
        return Err("expected lowercase hexadecimal characters");
    }
    Ok(())
}

fn validate_evidence_id(value: &str) -> Result<(), &'static str> {
    let mut parts = value.split(':');
    if parts.next() != Some("evidence") {
        return Err("expected the evidence namespace");
    }
    let remainder: Vec<_> = parts.collect();
    if remainder.len() < 2 || !remainder.iter().all(|part| is_safe_component(part)) {
        return Err("expected at least two safe identifier components");
    }
    Ok(())
}

fn validate_analysis_version(value: &str) -> Result<(), &'static str> {
    if !is_safe_component(value) {
        return Err("expected a canonical lowercase version identifier");
    }
    Ok(())
}

fn validate_locator_component(value: &str) -> Result<(), &'static str> {
    if !is_safe_component(value) {
        return Err("expected a canonical portable repository component");
    }
    Ok(())
}

fn validate_machine_code(value: &str) -> Result<(), &'static str> {
    if value.is_empty() || value.len() > MAX_CODE_LENGTH {
        return Err("expected a non-empty snake_case code of at most 64 characters");
    }
    let mut bytes = value.bytes();
    if !bytes.next().is_some_and(|byte| byte.is_ascii_lowercase()) {
        return Err("expected a snake_case code beginning with a letter");
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        || value.ends_with('_')
        || value.contains("__")
    {
        return Err("expected a canonical snake_case code");
    }
    Ok(())
}

fn is_safe_component(value: &str) -> bool {
    if value.is_empty() || value.len() > MAX_COMPONENT_LENGTH || value.contains("..") {
        return false;
    }
    let bytes = value.as_bytes();
    let is_boundary = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit();
    is_boundary(bytes[0])
        && is_boundary(bytes[bytes.len() - 1])
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}

fn is_lower_hex(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

validated_string_value!(RevisionId, "revision_id", validate_revision);
validated_string_value!(ContentHash, "content_hash", validate_sha256);
validated_string_value!(EvidenceId, "evidence_id", validate_evidence_id);
validated_string_value!(
    AnalysisVersion,
    "analysis_version",
    validate_analysis_version
);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct LocatorComponent(String);

impl TryFrom<String> for LocatorComponent {
    type Error = DomainValueError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        validate_locator_component(&value)
            .map_err(|reason| DomainValueError::new("repository_source", reason))?;
        Ok(Self(value))
    }
}

impl Serialize for LocatorComponent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for LocatorComponent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_from(String::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct MachineCode(String);

impl TryFrom<String> for MachineCode {
    type Error = DomainValueError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        validate_machine_code(&value)
            .map_err(|reason| DomainValueError::new("machine_code", reason))?;
        Ok(Self(value))
    }
}

impl FromStr for MachineCode {
    type Err = DomainValueError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_from(value.to_owned())
    }
}

impl Serialize for MachineCode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for MachineCode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_from(String::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

/// A SHA-256 digest of the complete effective analysis rule set.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuleSetHash(ContentHash);

impl RuleSetHash {
    /// Returns the canonical `sha256:<digest>` representation.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl FromStr for RuleSetHash {
    type Err = DomainValueError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        ContentHash::from_str(value)
            .map(Self)
            .map_err(|error| DomainValueError::new("rule_set_hash", error.reason()))
    }
}

impl TryFrom<String> for RuleSetHash {
    type Error = DomainValueError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        ContentHash::try_from(value)
            .map(Self)
            .map_err(|error| DomainValueError::new("rule_set_hash", error.reason()))
    }
}

impl Serialize for RuleSetHash {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for RuleSetHash {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_from(String::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum RepositorySourceData {
    Local {
        repository_id: ContentHash,
    },
    Hosted {
        provider: LocatorComponent,
        namespace: LocatorComponent,
        repository: LocatorComponent,
    },
}

/// A portable repository locator that cannot contain a local filesystem path.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct RepositorySource(RepositorySourceData);

impl RepositorySource {
    /// Creates a local source identified by a content-derived identifier.
    pub const fn local(repository_id: ContentHash) -> Self {
        Self(RepositorySourceData::Local { repository_id })
    }

    /// Creates a canonical provider-neutral hosted repository locator.
    pub fn hosted(
        provider: &str,
        namespace: &str,
        repository: &str,
    ) -> Result<Self, DomainValueError> {
        Ok(Self(RepositorySourceData::Hosted {
            provider: LocatorComponent::try_from(provider.to_owned())?,
            namespace: LocatorComponent::try_from(namespace.to_owned())?,
            repository: LocatorComponent::try_from(repository.to_owned())?,
        }))
    }

    /// Returns the content-derived local repository identifier, when local.
    pub const fn local_repository_id(&self) -> Option<&ContentHash> {
        match &self.0 {
            RepositorySourceData::Local { repository_id } => Some(repository_id),
            RepositorySourceData::Hosted { .. } => None,
        }
    }

    /// Returns canonical provider, namespace, and repository components, when hosted.
    pub fn hosted_locator(&self) -> Option<(&str, &str, &str)> {
        match &self.0 {
            RepositorySourceData::Hosted {
                provider,
                namespace,
                repository,
            } => Some((&provider.0, &namespace.0, &repository.0)),
            RepositorySourceData::Local { .. } => None,
        }
    }
}

/// Availability of one evidence source, independent from analysis status.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceStatus {
    /// The requested evidence was collected completely.
    Complete,
    /// Some usable evidence was collected, with explicit gaps.
    Partial,
    /// The evidence could not be obtained from the requested source.
    Unavailable,
    /// The analyzer does not support this evidence source or content.
    Unsupported,
    /// Evidence exists but is not sufficient for the requested interpretation.
    Insufficient,
    /// Evidence collection or maturation is not final yet.
    Pending,
}

/// Overall derived-analysis state, kept separate from raw evidence availability.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisStatus {
    /// All requested analysis completed.
    Complete,
    /// A usable result completed with explicit gaps.
    Partial,
    /// Analysis could not produce a usable result because required input was unavailable.
    Unavailable,
    /// The requested analysis is not supported.
    Unsupported,
    /// Collected inputs are insufficient for the requested analysis.
    Insufficient,
    /// Analysis or maturation is not final yet.
    Pending,
}

/// A stable category for evidence provenance.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceSourceKind {
    Repository,
    RepositoryContent,
    RepositoryHistory,
    PlatformRecord,
    ReportedCi,
    ReleaseArtifact,
    Documentation,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EvidenceSourceData {
    id: EvidenceId,
    kind: EvidenceSourceKind,
    status: EvidenceStatus,
    revision: Option<RevisionId>,
    content_hash: Option<ContentHash>,
}

/// Provenance and availability for one stable evidence identifier.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EvidenceSource {
    id: EvidenceId,
    kind: EvidenceSourceKind,
    status: EvidenceStatus,
    revision: Option<RevisionId>,
    content_hash: Option<ContentHash>,
}

impl EvidenceSource {
    /// Creates evidence pinned to an immutable source revision.
    pub const fn at_revision(
        id: EvidenceId,
        kind: EvidenceSourceKind,
        status: EvidenceStatus,
        revision: RevisionId,
    ) -> Self {
        Self {
            id,
            kind,
            status,
            revision: Some(revision),
            content_hash: None,
        }
    }

    /// Creates content evidence pinned to both a revision and SHA-256 digest.
    pub const fn at_content(
        id: EvidenceId,
        kind: EvidenceSourceKind,
        status: EvidenceStatus,
        revision: RevisionId,
        content_hash: ContentHash,
    ) -> Self {
        Self {
            id,
            kind,
            status,
            revision: Some(revision),
            content_hash: Some(content_hash),
        }
    }

    /// Creates an explicit unresolved source for a non-usable evidence state.
    pub fn unresolved(
        id: EvidenceId,
        kind: EvidenceSourceKind,
        status: EvidenceStatus,
    ) -> Result<Self, DomainValueError> {
        Self::validate_provenance(status, None, None)?;
        Ok(Self {
            id,
            kind,
            status,
            revision: None,
            content_hash: None,
        })
    }

    /// Returns the stable evidence identifier.
    pub const fn id(&self) -> &EvidenceId {
        &self.id
    }

    /// Returns the stable provenance category.
    pub const fn kind(&self) -> EvidenceSourceKind {
        self.kind
    }

    /// Returns availability without inferring the overall analysis state.
    pub const fn status(&self) -> EvidenceStatus {
        self.status
    }

    /// Returns the immutable source revision when known.
    pub const fn revision(&self) -> Option<&RevisionId> {
        self.revision.as_ref()
    }

    /// Returns the content digest when evidence was content-addressed.
    pub const fn content_hash(&self) -> Option<&ContentHash> {
        self.content_hash.as_ref()
    }

    fn validate_provenance(
        status: EvidenceStatus,
        revision: Option<&RevisionId>,
        content_hash: Option<&ContentHash>,
    ) -> Result<(), DomainValueError> {
        if matches!(status, EvidenceStatus::Complete | EvidenceStatus::Partial)
            && revision.is_none()
            && content_hash.is_none()
        {
            return Err(DomainValueError::new(
                "evidence_source",
                "complete or partial evidence requires immutable provenance",
            ));
        }
        Ok(())
    }
}

impl TryFrom<EvidenceSourceData> for EvidenceSource {
    type Error = DomainValueError;

    fn try_from(value: EvidenceSourceData) -> Result<Self, Self::Error> {
        Self::validate_provenance(
            value.status,
            value.revision.as_ref(),
            value.content_hash.as_ref(),
        )?;
        Ok(Self {
            id: value.id,
            kind: value.kind,
            status: value.status,
            revision: value.revision,
            content_hash: value.content_hash,
        })
    }
}

impl<'de> Deserialize<'de> for EvidenceSource {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_from(EvidenceSourceData::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

/// An immutable repository snapshot used as an analysis input.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceSnapshot {
    source: RepositorySource,
    revision: RevisionId,
    root_tree: Option<RevisionId>,
}

impl SourceSnapshot {
    /// Creates a snapshot pinned to a full revision and optional root tree ID.
    pub const fn new(
        source: RepositorySource,
        revision: RevisionId,
        root_tree: Option<RevisionId>,
    ) -> Self {
        Self {
            source,
            revision,
            root_tree,
        }
    }

    /// Returns the portable repository source.
    pub const fn source(&self) -> &RepositorySource {
        &self.source
    }

    /// Returns the immutable analyzed revision.
    pub const fn revision(&self) -> &RevisionId {
        &self.revision
    }

    /// Returns the immutable root tree ID when it was available.
    pub const fn root_tree(&self) -> Option<&RevisionId> {
        self.root_tree.as_ref()
    }
}

/// A machine-readable warning code without free-form sensitive data.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Warning {
    code: MachineCode,
}

impl Warning {
    /// Creates a warning from a canonical snake_case code.
    pub fn new(code: &str) -> Result<Self, DomainValueError> {
        Ok(Self {
            code: MachineCode::from_str(code)?,
        })
    }

    /// Returns the stable warning code.
    pub fn code(&self) -> &str {
        &self.code.0
    }
}

/// A machine-readable limitation code without free-form sensitive data.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Limitation {
    code: MachineCode,
}

impl Limitation {
    /// Creates a limitation from a canonical snake_case code.
    pub fn new(code: &str) -> Result<Self, DomainValueError> {
        Ok(Self {
            code: MachineCode::from_str(code)?,
        })
    }

    /// Returns the stable limitation code.
    pub fn code(&self) -> &str {
        &self.code.0
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AnalysisManifestData {
    source_snapshot: SourceSnapshot,
    analysis_version: AnalysisVersion,
    rule_set_hash: RuleSetHash,
    status: AnalysisStatus,
    evidence_sources: Vec<EvidenceSource>,
    warnings: Vec<Warning>,
    limitations: Vec<Limitation>,
}

/// Deterministic domain manifest for one immutable source snapshot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AnalysisManifest {
    source_snapshot: SourceSnapshot,
    analysis_version: AnalysisVersion,
    rule_set_hash: RuleSetHash,
    status: AnalysisStatus,
    evidence_sources: Vec<EvidenceSource>,
    warnings: Vec<Warning>,
    limitations: Vec<Limitation>,
}

impl AnalysisManifest {
    /// Creates a manifest and canonicalizes all identifier-keyed collections.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source_snapshot: SourceSnapshot,
        analysis_version: AnalysisVersion,
        rule_set_hash: RuleSetHash,
        status: AnalysisStatus,
        mut evidence_sources: Vec<EvidenceSource>,
        mut warnings: Vec<Warning>,
        mut limitations: Vec<Limitation>,
    ) -> Result<Self, DomainValueError> {
        if evidence_sources.is_empty() {
            return Err(DomainValueError::new(
                "analysis_manifest",
                "at least one explicit evidence source is required",
            ));
        }
        if status == AnalysisStatus::Complete
            && evidence_sources
                .iter()
                .any(|source| source.status != EvidenceStatus::Complete)
        {
            return Err(DomainValueError::new(
                "analysis_manifest",
                "complete analysis requires every evidence source to be complete",
            ));
        }

        evidence_sources.sort_by(|left, right| left.id.cmp(&right.id));
        if evidence_sources
            .windows(2)
            .any(|pair| pair[0].id == pair[1].id)
        {
            return Err(DomainValueError::new(
                "analysis_manifest",
                "evidence identifiers must be unique",
            ));
        }

        warnings.sort_by(|left, right| left.code.cmp(&right.code));
        if warnings.windows(2).any(|pair| pair[0].code == pair[1].code) {
            return Err(DomainValueError::new(
                "analysis_manifest",
                "warning codes must be unique",
            ));
        }

        limitations.sort_by(|left, right| left.code.cmp(&right.code));
        if limitations
            .windows(2)
            .any(|pair| pair[0].code == pair[1].code)
        {
            return Err(DomainValueError::new(
                "analysis_manifest",
                "limitation codes must be unique",
            ));
        }

        Ok(Self {
            source_snapshot,
            analysis_version,
            rule_set_hash,
            status,
            evidence_sources,
            warnings,
            limitations,
        })
    }

    /// Returns the overall analysis state without changing evidence states.
    pub const fn status(&self) -> AnalysisStatus {
        self.status
    }

    /// Returns the immutable source snapshot.
    pub const fn source_snapshot(&self) -> &SourceSnapshot {
        &self.source_snapshot
    }

    /// Returns the analysis contract version.
    pub const fn analysis_version(&self) -> &AnalysisVersion {
        &self.analysis_version
    }

    /// Returns the hash of the complete effective rule set.
    pub const fn rule_set_hash(&self) -> &RuleSetHash {
        &self.rule_set_hash
    }

    /// Returns evidence sources in canonical evidence-ID order.
    pub fn evidence_sources(&self) -> &[EvidenceSource] {
        &self.evidence_sources
    }

    /// Returns warnings in canonical code order.
    pub fn warnings(&self) -> &[Warning] {
        &self.warnings
    }

    /// Returns limitations in canonical code order.
    pub fn limitations(&self) -> &[Limitation] {
        &self.limitations
    }
}

impl TryFrom<AnalysisManifestData> for AnalysisManifest {
    type Error = DomainValueError;

    fn try_from(value: AnalysisManifestData) -> Result<Self, Self::Error> {
        Self::new(
            value.source_snapshot,
            value.analysis_version,
            value.rule_set_hash,
            value.status,
            value.evidence_sources,
            value.warnings,
            value.limitations,
        )
    }
}

impl<'de> Deserialize<'de> for AnalysisManifest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_from(AnalysisManifestData::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}
