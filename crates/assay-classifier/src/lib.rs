//! Versioned, path-based file policy classification for Assay.
//!
//! The built-in policy measures reviewable path and resolved Git attribute
//! evidence. It does not inspect source contents, execute repository code, or
//! measure correctness, importance, human effort, productivity, or semantic
//! impact. A category describes the apparent role of a file; it is not a
//! quality judgment. In particular, [`ClassificationCategory::Unknown`] and
//! unavailable attribute facts must not be interpreted as zero value or
//! silently converted to production code.
//!
//! Repository-specific and organization-specific policy belongs behind the
//! [`ClassificationPolicy`] boundary. It is not embedded in the built-in Rust
//! rules.

#![forbid(unsafe_code)]

use std::{collections::BTreeSet, error::Error, fmt};

/// Stable package identifier for diagnostics and capability reporting.
pub const PACKAGE_NAME: &str = env!("CARGO_PKG_NAME");

/// Stable version of the complete built-in file classification policy.
pub const BUILT_IN_RULE_SET_VERSION: &str = "file-classifier-1";

/// A validation error that does not retain or echo rejected path input.
#[derive(Clone, Eq, PartialEq)]
pub struct ClassificationError {
    value_kind: &'static str,
    reason: &'static str,
}

impl ClassificationError {
    fn portable_path(reason: &'static str) -> Self {
        Self {
            value_kind: "portable_path",
            reason,
        }
    }

    fn rule_id(reason: &'static str) -> Self {
        Self {
            value_kind: "rule_id",
            reason,
        }
    }

    fn confidence(reason: &'static str) -> Self {
        Self {
            value_kind: "confidence",
            reason,
        }
    }

    fn policy_version(reason: &'static str) -> Self {
        Self {
            value_kind: "policy_version",
            reason,
        }
    }

    /// Returns the stable input kind that failed validation.
    pub const fn value_kind(&self) -> &'static str {
        self.value_kind
    }

    /// Returns a non-sensitive reason that never includes the rejected value.
    pub const fn reason(&self) -> &'static str {
        self.reason
    }
}

impl fmt::Debug for ClassificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ClassificationError")
            .field("value_kind", &self.value_kind)
            .field("reason", &self.reason)
            .finish()
    }
}

impl fmt::Display for ClassificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid {}: {}", self.value_kind, self.reason)
    }
}

impl Error for ClassificationError {}

/// A repository-relative UTF-8 path using `/` separators.
///
/// Absolute paths, traversal components, repeated separators, NUL bytes, and
/// platform-specific separators are rejected. Spaces, Unicode, and ASCII case
/// variants are preserved. Consumers should avoid logging this value because a
/// repository path can itself contain private information.
#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PortablePath(String);

impl PortablePath {
    /// Returns the validated repository-relative path.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn lowercase_components(&self) -> Vec<String> {
        self.0
            .split('/')
            .map(|component| component.to_ascii_lowercase())
            .collect()
    }
}

impl fmt::Debug for PortablePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PortablePath(<redacted>)")
    }
}

impl TryFrom<&str> for PortablePath {
    type Error = ClassificationError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        validate_portable_path(value)?;
        Ok(Self(value.to_owned()))
    }
}

impl TryFrom<String> for PortablePath {
    type Error = ClassificationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        validate_portable_path(&value)?;
        Ok(Self(value))
    }
}

fn validate_portable_path(value: &str) -> Result<(), ClassificationError> {
    if value.is_empty() {
        return Err(ClassificationError::portable_path(
            "expected a non-empty path",
        ));
    }
    if value.contains('\0') {
        return Err(ClassificationError::portable_path(
            "NUL bytes are not allowed",
        ));
    }
    if value.starts_with('/') || value.starts_with('\\') || has_windows_drive_prefix(value) {
        return Err(ClassificationError::portable_path(
            "absolute paths are not allowed",
        ));
    }
    if value.contains('\\') {
        return Err(ClassificationError::portable_path(
            "expected portable forward-slash separators",
        ));
    }
    if value
        .split('/')
        .any(|component| component.is_empty() || matches!(component, "." | ".."))
    {
        return Err(ClassificationError::portable_path(
            "empty and traversal components are not allowed",
        ));
    }
    Ok(())
}

fn has_windows_drive_prefix(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

/// Availability of resolved `.gitattributes` facts for one file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttributeAvailability {
    /// Attribute resolution was performed, including when neither attribute
    /// was specified.
    Available,
    /// The adapter could not resolve attributes for this file.
    Unavailable,
}

/// Resolved GitHub Linguist attributes for one file.
///
/// Git-specific parsing remains outside this crate. A Git adapter resolves
/// `.gitattributes` precedence and passes the resulting optional booleans into
/// this domain input contract. `None` means the available attribute was not
/// specified; it is distinct from unavailable attribute resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LinguistAttributeFacts {
    availability: AttributeAvailability,
    generated: Option<bool>,
    vendored: Option<bool>,
}

impl LinguistAttributeFacts {
    /// Creates available facts from resolved `linguist-generated` and
    /// `linguist-vendored` values.
    pub const fn available(generated: Option<bool>, vendored: Option<bool>) -> Self {
        Self {
            availability: AttributeAvailability::Available,
            generated,
            vendored,
        }
    }

    /// Creates an explicit unavailable state without inventing false values.
    pub const fn unavailable() -> Self {
        Self {
            availability: AttributeAvailability::Unavailable,
            generated: None,
            vendored: None,
        }
    }

    /// Returns whether attribute resolution was available.
    pub const fn availability(self) -> AttributeAvailability {
        self.availability
    }

    /// Returns the resolved `linguist-generated` value when specified.
    pub const fn generated(self) -> Option<bool> {
        self.generated
    }

    /// Returns the resolved `linguist-vendored` value when specified.
    pub const fn vendored(self) -> Option<bool> {
        self.vendored
    }
}

/// Validated facts consumed by a classification policy.
pub struct FileClassificationInput {
    path: PortablePath,
    attributes: LinguistAttributeFacts,
}

impl FileClassificationInput {
    /// Validates a repository-relative path and combines it with already
    /// resolved Git attribute facts.
    pub fn try_new(
        path: impl TryInto<PortablePath, Error = ClassificationError>,
        attributes: LinguistAttributeFacts,
    ) -> Result<Self, ClassificationError> {
        Ok(Self {
            path: path.try_into()?,
            attributes,
        })
    }

    /// Returns the portable path for policy evaluation.
    pub const fn path(&self) -> &PortablePath {
        &self.path
    }

    /// Returns resolved Linguist attribute facts.
    pub const fn attributes(&self) -> LinguistAttributeFacts {
        self.attributes
    }
}

impl fmt::Debug for FileClassificationInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FileClassificationInput")
            .field("path", &self.path)
            .field("attributes", &self.attributes)
            .finish()
    }
}

/// Primary role assigned to one repository file.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ClassificationCategory {
    /// Application, library, or other implementation source.
    ProductionCode,
    /// Automated or manual test source and fixtures.
    Test,
    /// Documentation and community-facing text.
    Documentation,
    /// Continuous integration and delivery configuration.
    CiCd,
    /// Deployment and infrastructure-as-code material.
    Infrastructure,
    /// Versioned database or schema migration material.
    SchemaMigration,
    /// Dependency manifests and lockfiles.
    Dependency,
    /// Repository security policy or security automation configuration.
    SecurityPolicy,
    /// General project or tool configuration.
    Configuration,
    /// Generated or minified material.
    Generated,
    /// Vendored or third-party material.
    Vendored,
    /// Build output or compiled artifacts.
    BuildOutput,
    /// Coverage output.
    Coverage,
    /// No built-in path rule supplied sufficient evidence.
    Unknown,
}

/// Optional facts retained alongside the primary category.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ClassificationTag {
    /// The file is a dependency manifest.
    DependencyManifest,
    /// The file is a dependency lockfile.
    Lockfile,
    /// `linguist-generated=true` contributed to classification.
    LinguistGenerated,
    /// `linguist-vendored=true` contributed to classification.
    LinguistVendored,
    /// `linguist-generated=false` suppressed a matching built-in rule.
    GeneratedSuppressed,
    /// `linguist-vendored=false` suppressed a matching built-in rule.
    VendoredSuppressed,
    /// Resolved Git attribute facts were unavailable.
    AttributesUnavailable,
    /// A minified filename supplied generated-file evidence.
    Minified,
}

/// A stable rule identifier scoped by a [`PolicyVersion`].
///
/// Individual rule IDs need not repeat the policy version. The policy identity
/// carried by every [`FileClassification`] versions their meaning.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuleId(String);

impl RuleId {
    fn built_in(value: &'static str) -> Self {
        Self(value.to_owned())
    }

    /// Creates a policy rule identifier suitable for external versioned
    /// policy adapters.
    pub fn try_new(value: impl Into<String>) -> Result<Self, ClassificationError> {
        let value = value.into();
        if value.is_empty() || value.len() > 128 {
            return Err(ClassificationError::rule_id(
                "expected a non-empty identifier of at most 128 characters",
            ));
        }
        let bytes = value.as_bytes();
        if !bytes[0].is_ascii_lowercase()
            || !bytes[bytes.len() - 1].is_ascii_lowercase()
                && !bytes[bytes.len() - 1].is_ascii_digit()
            || value.contains("..")
            || !bytes.iter().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'.' | b'_' | b'-')
            })
        {
            return Err(ClassificationError::rule_id(
                "expected a canonical lowercase versioned identifier",
            ));
        }
        Ok(Self(value))
    }

    /// Returns the canonical rule identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Validated identity of one complete classification policy.
///
/// A canonical policy identity ends in a positive numeric version, such as
/// `file-classifier-1` or `deployment-policy-v7`. Future CFG-002 rule-set
/// hashing can combine this identity with normalized external policy inputs;
/// this identity is provenance and is not itself a configuration hash.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PolicyVersion(String);

impl PolicyVersion {
    fn built_in() -> Self {
        Self(BUILT_IN_RULE_SET_VERSION.to_owned())
    }

    /// Creates an explicitly versioned canonical policy identity.
    pub fn try_new(value: impl Into<String>) -> Result<Self, ClassificationError> {
        let value = value.into();
        if value.is_empty() || value.len() > 128 {
            return Err(ClassificationError::policy_version(
                "expected a non-empty identity of at most 128 characters",
            ));
        }
        let bytes = value.as_bytes();
        if !bytes[0].is_ascii_lowercase()
            || !bytes[bytes.len() - 1].is_ascii_digit()
            || !bytes.iter().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'.' | b'_' | b'-')
            })
        {
            return Err(ClassificationError::policy_version(
                "expected a canonical lowercase identity",
            ));
        }
        let version = value.rsplit('-').next().unwrap_or_default();
        let digits = version.strip_prefix('v').unwrap_or(version);
        if digits.is_empty()
            || digits.starts_with('0')
            || !digits.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(ClassificationError::policy_version(
                "expected a positive numeric version suffix",
            ));
        }
        Ok(Self(value))
    }

    /// Returns the canonical policy identity.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Confidence expressed as integer basis points from 0 through 10,000.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Confidence(u16);

impl Confidence {
    const CERTAIN: Self = Self(10_000);
    const HIGH: Self = Self(9_500);
    const MEDIUM_HIGH: Self = Self(8_500);
    const MEDIUM: Self = Self(7_500);
    const LOW: Self = Self(5_000);

    /// Creates confidence from integer basis points in the inclusive range
    /// 0 through 10,000.
    pub fn try_from_basis_points(value: u16) -> Result<Self, ClassificationError> {
        if value > 10_000 {
            return Err(ClassificationError::confidence(
                "expected at most 10000 basis points",
            ));
        }
        Ok(Self(value))
    }

    /// Returns confidence in basis points, where 10,000 is 1.0.
    pub const fn basis_points(self) -> u16 {
        self.0
    }
}

/// Kind of provenance retained by a classification result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClassificationEvidenceKind {
    /// A named versioned policy rule matched input facts.
    PolicyRule,
    /// A resolved `.gitattributes` Linguist value was applied.
    LinguistAttribute,
    /// Attribute resolution was explicitly unavailable.
    AttributeFactsUnavailable,
}

/// Non-sensitive provenance for a classification decision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassificationEvidence {
    kind: ClassificationEvidenceKind,
    rule_id: RuleId,
    attribute_name: Option<&'static str>,
    attribute_value: Option<bool>,
}

impl ClassificationEvidence {
    fn built_in(rule_id: RuleId) -> Self {
        Self {
            kind: ClassificationEvidenceKind::PolicyRule,
            rule_id,
            attribute_name: None,
            attribute_value: None,
        }
    }

    /// Creates non-sensitive evidence for an external versioned policy rule.
    pub fn policy_rule(rule_id: RuleId) -> Self {
        Self {
            kind: ClassificationEvidenceKind::PolicyRule,
            rule_id,
            attribute_name: None,
            attribute_value: None,
        }
    }

    fn attribute(rule_id: RuleId, name: &'static str, value: bool) -> Self {
        Self {
            kind: ClassificationEvidenceKind::LinguistAttribute,
            rule_id,
            attribute_name: Some(name),
            attribute_value: Some(value),
        }
    }

    fn unavailable() -> Self {
        Self {
            kind: ClassificationEvidenceKind::AttributeFactsUnavailable,
            rule_id: RuleId::built_in("classifier.v1.attributes.unavailable"),
            attribute_name: None,
            attribute_value: None,
        }
    }

    /// Returns the provenance kind.
    pub const fn kind(&self) -> ClassificationEvidenceKind {
        self.kind
    }

    /// Returns the rule that supplied this evidence.
    pub const fn rule_id(&self) -> &RuleId {
        &self.rule_id
    }

    /// Returns a Linguist attribute name for attribute evidence.
    pub const fn attribute_name(&self) -> Option<&'static str> {
        self.attribute_name
    }

    /// Returns a Linguist attribute value for attribute evidence.
    pub const fn attribute_value(&self) -> Option<bool> {
        self.attribute_value
    }

    /// Returns true when this evidence preserves unavailable attribute facts.
    pub const fn is_unavailable(&self) -> bool {
        matches!(
            self.kind,
            ClassificationEvidenceKind::AttributeFactsUnavailable
        )
    }
}

/// Complete, explainable classification of one file.
///
/// This output measures policy evidence only. It cannot establish source
/// correctness, value, intent, semantic impact, or contributor performance.
#[derive(Clone, Eq, PartialEq)]
pub struct FileClassification {
    policy_version: PolicyVersion,
    category: ClassificationCategory,
    tags: Vec<ClassificationTag>,
    rule_id: RuleId,
    confidence: Confidence,
    evidence: Vec<ClassificationEvidence>,
    attribute_availability: AttributeAvailability,
}

/// Explainable decision returned by a classification policy.
///
/// The decision does not carry policy identity itself. Callers use
/// [`classify_with_policy`] so the policy's validated version is attached to
/// the final result and cannot be omitted by a policy implementation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassificationDecision {
    category: ClassificationCategory,
    tags: Vec<ClassificationTag>,
    rule_id: RuleId,
    confidence: Confidence,
    evidence: Vec<ClassificationEvidence>,
}

impl ClassificationDecision {
    /// Creates an explainable decision for a versioned policy adapter.
    ///
    /// Tags are sorted and deduplicated. The primary policy rule is retained
    /// as evidence automatically if the adapter does not supply it.
    pub fn new(
        category: ClassificationCategory,
        tags: impl IntoIterator<Item = ClassificationTag>,
        rule_id: RuleId,
        confidence: Confidence,
        evidence: impl IntoIterator<Item = ClassificationEvidence>,
    ) -> Self {
        let tags = tags
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let mut evidence = evidence.into_iter().collect::<Vec<_>>();
        if !evidence.iter().any(|item| item.rule_id() == &rule_id) {
            evidence.push(ClassificationEvidence::policy_rule(rule_id.clone()));
        }
        Self {
            category,
            tags,
            rule_id,
            confidence,
            evidence,
        }
    }

    fn built_in(
        category: ClassificationCategory,
        rule_id: &'static str,
        confidence: Confidence,
    ) -> Self {
        Self {
            category,
            tags: Vec::new(),
            rule_id: RuleId::built_in(rule_id),
            confidence,
            evidence: Vec::new(),
        }
    }

    fn tagged(mut self, tag: ClassificationTag) -> Self {
        self.tags.push(tag);
        self
    }
}

impl FileClassification {
    fn from_decision(
        policy_version: PolicyVersion,
        decision: ClassificationDecision,
        attribute_availability: AttributeAvailability,
    ) -> Self {
        Self {
            policy_version,
            category: decision.category,
            tags: decision.tags,
            rule_id: decision.rule_id,
            confidence: decision.confidence,
            evidence: decision.evidence,
            attribute_availability,
        }
    }

    /// Returns the validated identity of the complete policy that produced
    /// this result.
    pub const fn policy_version(&self) -> &PolicyVersion {
        &self.policy_version
    }

    /// Returns the single primary category.
    pub const fn category(&self) -> ClassificationCategory {
        self.category
    }

    /// Returns stable, sorted secondary tags.
    pub fn tags(&self) -> &[ClassificationTag] {
        &self.tags
    }

    /// Returns the primary versioned rule identifier.
    pub const fn rule_id(&self) -> &RuleId {
        &self.rule_id
    }

    /// Returns policy confidence, not a quality score.
    pub const fn confidence(&self) -> Confidence {
        self.confidence
    }

    /// Returns non-sensitive rule and attribute provenance.
    pub fn evidence(&self) -> &[ClassificationEvidence] {
        &self.evidence
    }

    /// Returns whether resolved Git attribute facts were available.
    pub const fn attribute_availability(&self) -> AttributeAvailability {
        self.attribute_availability
    }
}

impl fmt::Debug for FileClassification {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FileClassification")
            .field("policy_version", &self.policy_version)
            .field("category", &self.category)
            .field("tags", &self.tags)
            .field("rule_id", &self.rule_id)
            .field("confidence", &self.confidence)
            .field("evidence", &self.evidence)
            .field("attribute_availability", &self.attribute_availability)
            .finish()
    }
}

/// Adapter boundary for built-in or externally configured versioned policy.
///
/// Future repository, organization, or deployment policy adapters implement
/// this trait. The built-in policy intentionally contains no project-specific
/// names or organization-specific exceptions.
pub trait ClassificationPolicy {
    /// Returns this policy's validated, explicit version identity.
    fn policy_version(&self) -> PolicyVersion;

    /// Evaluates a validated file input without I/O.
    fn evaluate(&self, input: &FileClassificationInput) -> ClassificationDecision;
}

/// Evaluates a policy and attaches its validated version to the result.
///
/// This is the enforced entry point for external policies: implementations
/// return only a [`ClassificationDecision`], while this function preserves the
/// policy identity and input availability in [`FileClassification`].
pub fn classify_with_policy(
    policy: &(impl ClassificationPolicy + ?Sized),
    input: &FileClassificationInput,
) -> FileClassification {
    FileClassification::from_decision(
        policy.policy_version(),
        policy.evaluate(input),
        input.attributes().availability(),
    )
}

/// Built-in Assay file classification policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuiltInPolicy {
    /// Initial versioned path and Linguist-attribute policy.
    V1,
}

impl BuiltInPolicy {
    /// Returns the stable version for this policy.
    pub fn version(self) -> PolicyVersion {
        match self {
            Self::V1 => PolicyVersion::built_in(),
        }
    }

    /// Classifies one file through the same version-preserving entry point
    /// used by external policies.
    pub fn classify(self, input: &FileClassificationInput) -> FileClassification {
        classify_with_policy(&self, input)
    }
}

impl ClassificationPolicy for BuiltInPolicy {
    fn policy_version(&self) -> PolicyVersion {
        self.version()
    }

    fn evaluate(&self, input: &FileClassificationInput) -> ClassificationDecision {
        match self {
            Self::V1 => evaluate_v1(input),
        }
    }
}

fn evaluate_v1(input: &FileClassificationInput) -> ClassificationDecision {
    let attributes = input.attributes();
    let mut tags = BTreeSet::new();
    let mut evidence = Vec::new();

    match attributes.availability() {
        AttributeAvailability::Available => {
            if let Some(value) = attributes.generated() {
                evidence.push(ClassificationEvidence::attribute(
                    RuleId::built_in("classifier.v1.attribute.generated"),
                    "linguist-generated",
                    value,
                ));
                if value {
                    tags.insert(ClassificationTag::LinguistGenerated);
                }
            }
            if let Some(value) = attributes.vendored() {
                evidence.push(ClassificationEvidence::attribute(
                    RuleId::built_in("classifier.v1.attribute.vendored"),
                    "linguist-vendored",
                    value,
                ));
                if value {
                    tags.insert(ClassificationTag::LinguistVendored);
                }
            }
        }
        AttributeAvailability::Unavailable => {
            tags.insert(ClassificationTag::AttributesUnavailable);
            evidence.push(ClassificationEvidence::unavailable());
        }
    }

    // When both attributes are true, generated is the deterministic primary
    // category and vendored remains visible as a secondary tag and evidence.
    let decision = if attributes.generated() == Some(true) {
        ClassificationDecision::built_in(
            ClassificationCategory::Generated,
            "classifier.v1.attribute.generated",
            Confidence::CERTAIN,
        )
    } else if attributes.vendored() == Some(true) {
        ClassificationDecision::built_in(
            ClassificationCategory::Vendored,
            "classifier.v1.attribute.vendored",
            Confidence::CERTAIN,
        )
    } else {
        let generated_rules_enabled = attributes.generated() != Some(false);
        let vendored_rules_enabled = attributes.vendored() != Some(false);
        let decision = classify_path_v1(
            input.path(),
            generated_rules_enabled,
            vendored_rules_enabled,
        );
        if attributes.generated() == Some(false)
            && classify_path_v1(input.path(), true, vendored_rules_enabled).category
                == ClassificationCategory::Generated
            && decision.category != ClassificationCategory::Generated
        {
            tags.insert(ClassificationTag::GeneratedSuppressed);
        }
        if attributes.vendored() == Some(false)
            && classify_path_v1(input.path(), generated_rules_enabled, true).category
                == ClassificationCategory::Vendored
            && decision.category != ClassificationCategory::Vendored
        {
            tags.insert(ClassificationTag::VendoredSuppressed);
        }
        decision
    };

    tags.extend(decision.tags);
    if !evidence
        .iter()
        .any(|item| item.rule_id() == &decision.rule_id)
    {
        evidence.push(ClassificationEvidence::built_in(decision.rule_id.clone()));
    }
    let confidence = if matches!(
        attributes.availability(),
        AttributeAvailability::Unavailable
    ) {
        decision.confidence.min(Confidence::MEDIUM)
    } else {
        decision.confidence
    };

    ClassificationDecision {
        category: decision.category,
        tags: tags.into_iter().collect(),
        rule_id: decision.rule_id,
        confidence,
        evidence,
    }
}

fn classify_path_v1(
    path: &PortablePath,
    generated_rules_enabled: bool,
    vendored_rules_enabled: bool,
) -> ClassificationDecision {
    let components = path.lowercase_components();
    let filename = components.last().map(String::as_str).unwrap_or_default();

    if is_coverage(&components, filename) {
        return ClassificationDecision::built_in(
            ClassificationCategory::Coverage,
            "classifier.v1.coverage",
            Confidence::HIGH,
        );
    }
    if is_build_output(&components, filename) {
        return ClassificationDecision::built_in(
            ClassificationCategory::BuildOutput,
            "classifier.v1.build_output",
            Confidence::HIGH,
        );
    }
    if generated_rules_enabled && is_generated(&components, filename) {
        let mut decision = ClassificationDecision::built_in(
            ClassificationCategory::Generated,
            "classifier.v1.generated",
            Confidence::HIGH,
        );
        if is_minified(filename) {
            decision = decision.tagged(ClassificationTag::Minified);
        }
        return decision;
    }
    if vendored_rules_enabled && is_vendored(&components) {
        return ClassificationDecision::built_in(
            ClassificationCategory::Vendored,
            "classifier.v1.vendored",
            Confidence::HIGH,
        );
    }
    if is_ci(&components, filename) {
        return ClassificationDecision::built_in(
            ClassificationCategory::CiCd,
            "classifier.v1.ci_cd",
            Confidence::HIGH,
        );
    }
    if is_schema_migration(&components, filename) {
        return ClassificationDecision::built_in(
            ClassificationCategory::SchemaMigration,
            "classifier.v1.schema_migration",
            Confidence::HIGH,
        );
    }
    if is_security_policy(&components, filename) {
        return ClassificationDecision::built_in(
            ClassificationCategory::SecurityPolicy,
            "classifier.v1.security_policy",
            Confidence::HIGH,
        );
    }
    if is_documentation(&components, filename) {
        return ClassificationDecision::built_in(
            ClassificationCategory::Documentation,
            "classifier.v1.documentation",
            Confidence::HIGH,
        );
    }
    if is_test(&components, filename) {
        return ClassificationDecision::built_in(
            ClassificationCategory::Test,
            "classifier.v1.test",
            Confidence::HIGH,
        );
    }
    if is_lockfile(filename) {
        return ClassificationDecision::built_in(
            ClassificationCategory::Dependency,
            "classifier.v1.dependency.lockfile",
            Confidence::HIGH,
        )
        .tagged(ClassificationTag::Lockfile);
    }
    if is_dependency_manifest(filename) {
        return ClassificationDecision::built_in(
            ClassificationCategory::Dependency,
            "classifier.v1.dependency.manifest",
            Confidence::HIGH,
        )
        .tagged(ClassificationTag::DependencyManifest);
    }
    if is_infrastructure(&components, filename) {
        return ClassificationDecision::built_in(
            ClassificationCategory::Infrastructure,
            "classifier.v1.infrastructure",
            Confidence::HIGH,
        );
    }
    if is_configuration(&components, filename) {
        return ClassificationDecision::built_in(
            ClassificationCategory::Configuration,
            "classifier.v1.configuration",
            Confidence::MEDIUM,
        );
    }
    if is_source(filename) {
        return ClassificationDecision::built_in(
            ClassificationCategory::ProductionCode,
            "classifier.v1.production_code",
            Confidence::MEDIUM_HIGH,
        );
    }
    ClassificationDecision::built_in(
        ClassificationCategory::Unknown,
        "classifier.v1.unknown",
        Confidence::LOW,
    )
}

fn contains_component(components: &[String], candidates: &[&str]) -> bool {
    components
        .iter()
        .any(|component| candidates.contains(&component.as_str()))
}

fn is_coverage(components: &[String], filename: &str) -> bool {
    contains_component(components, &["coverage", ".nyc_output", "htmlcov"])
        || matches!(filename, "lcov.info" | ".coverage" | "coverage.xml")
}

fn is_build_output(components: &[String], filename: &str) -> bool {
    contains_component(
        components,
        &["build", "dist", "out", "target", "bin", "obj"],
    ) || matches!(filename, "bundle.js" | "bundle.css")
}

fn is_generated(components: &[String], filename: &str) -> bool {
    contains_component(components, &["generated", "gen", "codegen"])
        || filename.contains(".generated.")
        || filename.ends_with("_pb.js")
        || filename.ends_with("_pb.d.ts")
        || filename.ends_with("_pb2.py")
        || filename.ends_with("_pb2.pyi")
        || filename.ends_with(".pb.go")
        || filename.ends_with(".g.cs")
        || is_minified(filename)
}

fn is_minified(filename: &str) -> bool {
    filename.contains(".min.js") || filename.contains(".min.css")
}

fn is_vendored(components: &[String]) -> bool {
    contains_component(
        components,
        &[
            "vendor",
            "vendored",
            "third_party",
            "third-party",
            "node_modules",
        ],
    )
}

fn is_ci(components: &[String], filename: &str) -> bool {
    (components.first().is_some_and(|first| first == ".github")
        && components
            .get(1)
            .is_some_and(|second| second == "workflows"))
        || contains_component(components, &[".circleci", ".buildkite"])
        || matches!(
            filename,
            ".gitlab-ci.yml"
                | ".gitlab-ci.yaml"
                | "jenkinsfile"
                | "azure-pipelines.yml"
                | "azure-pipelines.yaml"
        )
}

fn is_schema_migration(components: &[String], filename: &str) -> bool {
    contains_component(components, &["migration", "migrations"]) || filename.contains(".migration.")
}

fn is_security_policy(components: &[String], filename: &str) -> bool {
    matches!(
        filename,
        "security.md" | "security.txt" | "dependabot.yml" | "dependabot.yaml"
    ) || contains_component(components, &["security", "codeql"])
}

fn is_documentation(components: &[String], filename: &str) -> bool {
    contains_component(components, &["doc", "docs", "documentation"])
        || matches!(
            filename,
            "readme"
                | "readme.md"
                | "readme.rst"
                | "readme.txt"
                | "license"
                | "license.md"
                | "license.txt"
                | "copying"
                | "changelog"
                | "changelog.md"
                | "contributing.md"
                | "code_of_conduct.md"
        )
        || matches!(extension(filename), Some("md" | "mdx" | "rst" | "adoc"))
}

fn is_test(components: &[String], filename: &str) -> bool {
    contains_component(
        components,
        &["test", "tests", "__tests__", "spec", "specs", "fixtures"],
    ) || filename.starts_with("test_")
        || filename.contains(".test.")
        || filename.contains(".spec.")
        || filename.ends_with("_test.py")
        || filename.ends_with("_test.go")
}

fn is_lockfile(filename: &str) -> bool {
    matches!(
        filename,
        "bun.lock"
            | "cargo.lock"
            | "package-lock.json"
            | "npm-shrinkwrap.json"
            | "pnpm-lock.yaml"
            | "yarn.lock"
            | "poetry.lock"
            | "pdm.lock"
            | "pipfile.lock"
            | "uv.lock"
            | "composer.lock"
            | "gemfile.lock"
            | "go.sum"
    )
}

fn is_dependency_manifest(filename: &str) -> bool {
    matches!(
        filename,
        "cargo.toml"
            | "package.json"
            | "pyproject.toml"
            | "pipfile"
            | "poetry.toml"
            | "composer.json"
            | "gemfile"
            | "go.mod"
            | "pom.xml"
            | "build.gradle"
            | "build.gradle.kts"
    ) || (filename.starts_with("requirements") && filename.ends_with(".txt"))
}

fn is_infrastructure(components: &[String], filename: &str) -> bool {
    contains_component(
        components,
        &[
            "infra",
            "infrastructure",
            "terraform",
            "k8s",
            "kubernetes",
            "helm",
            "deploy",
            "deployment",
            "ansible",
        ],
    ) || filename == "dockerfile"
        || filename.starts_with("docker-compose.")
        || matches!(extension(filename), Some("tf" | "tfvars"))
}

fn is_configuration(components: &[String], filename: &str) -> bool {
    contains_component(components, &["config", "configuration", ".config"])
        || matches!(
            filename,
            ".gitattributes"
                | ".gitignore"
                | ".editorconfig"
                | ".prettierrc"
                | ".eslintrc"
                | "tsconfig.json"
                | "ruff.toml"
                | "mypy.ini"
        )
        || matches!(
            extension(filename),
            Some("toml" | "yaml" | "yml" | "json" | "ini" | "cfg" | "conf")
        )
}

fn is_source(filename: &str) -> bool {
    matches!(
        extension(filename),
        Some(
            "js" | "jsx"
                | "mjs"
                | "cjs"
                | "ts"
                | "tsx"
                | "py"
                | "pyi"
                | "rs"
                | "c"
                | "h"
                | "cc"
                | "cpp"
                | "hpp"
                | "go"
                | "java"
                | "kt"
                | "kts"
                | "rb"
                | "php"
                | "swift"
                | "cs"
                | "scala"
                | "sh"
                | "bash"
        )
    )
}

fn extension(filename: &str) -> Option<&str> {
    filename.rsplit_once('.').map(|(_, extension)| extension)
}
