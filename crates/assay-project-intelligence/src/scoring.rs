//! Deterministic project score compiler.
//!
//! The compiler combines deterministic per-dimension rule contributions with
//! validated qualitative rubric judgments into dimensioned, confidence-aware
//! scores that implement `schemas/project-evaluation/v1.json`. It performs no
//! filesystem, process, network, clock, or model-provider I/O; identical input
//! yields byte-identical output.
//!
//! A provider can influence a score only through a bounded [`RubricJudgment`]
//! rating; it can never emit or override a dimension or the overall Assay Score.
//! `not_applicable` and unavailable checks never become a zero score. Popularity
//! signals such as stars, forks, and downloads have no input to the compiler and
//! therefore cannot raise a score. Potential is compiled separately and is never
//! included in the Assay Score. Weights and the sufficiency rule are versioned
//! policy data folded into the published rule-set hash, not scattered constants.

use std::{collections::BTreeMap, error::Error, fmt};

use assay_domain::{
    EvidenceId, EvidenceStatus, RepositorySource, RevisionId, RubricApplicability,
    RubricCriterionId, RubricJudgmentSet,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const SCHEMA_VERSION: &str = "1.0.0";
const EVALUATION_VERSION: &str = "project-intelligence-1";
const RULE_SET_DOMAIN: &[u8] = b"assay.project-intelligence.score-compiler.rule-set.v1";
const MAX_STATEMENT_BYTES: usize = 1_000;

/// One of the five Assay Score dimensions or the separate Potential indicator.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ScoreDimension {
    ProjectSubstance,
    Originality,
    EngineeringRigor,
    OpenSourceReadiness,
    MaintenanceHealth,
    Potential,
}

/// Assay Score dimensions in canonical order; Potential is intentionally absent.
pub const ASSAY_SCORE_DIMENSIONS: [ScoreDimension; 5] = [
    ScoreDimension::ProjectSubstance,
    ScoreDimension::Originality,
    ScoreDimension::EngineeringRigor,
    ScoreDimension::OpenSourceReadiness,
    ScoreDimension::MaintenanceHealth,
];

impl ScoreDimension {
    /// Returns the stable machine field name used in the public contract.
    pub const fn field_name(self) -> &'static str {
        match self {
            Self::ProjectSubstance => "project_substance",
            Self::Originality => "originality",
            Self::EngineeringRigor => "engineering_rigor",
            Self::OpenSourceReadiness => "open_source_readiness",
            Self::MaintenanceHealth => "maintenance_health",
            Self::Potential => "potential",
        }
    }

    const fn criterion_prefix(self) -> &'static str {
        match self {
            Self::ProjectSubstance => "substance",
            Self::Originality => "originality",
            Self::EngineeringRigor => "engineering_rigor",
            Self::OpenSourceReadiness => "open_source_readiness",
            Self::MaintenanceHealth => "maintenance_health",
            Self::Potential => "potential",
        }
    }

    fn from_criterion_prefix(prefix: &str) -> Option<Self> {
        [
            Self::ProjectSubstance,
            Self::Originality,
            Self::EngineeringRigor,
            Self::OpenSourceReadiness,
            Self::MaintenanceHealth,
            Self::Potential,
        ]
        .into_iter()
        .find(|dimension| dimension.criterion_prefix() == prefix)
    }
}

/// Classified project type, matching the public evaluation contract.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProjectType {
    Application,
    LibrarySdkFramework,
    CliDeveloperTool,
    ServiceInfrastructurePlatform,
    CuratedResource,
    ProtocolSpecificationStandard,
    DatasetModelResearchArtifact,
    EducationalExampleTemplate,
    ExperimentalProofOfConcept,
}

impl ProjectType {
    const fn code(self) -> &'static str {
        match self {
            Self::Application => "application",
            Self::LibrarySdkFramework => "library_sdk_framework",
            Self::CliDeveloperTool => "cli_developer_tool",
            Self::ServiceInfrastructurePlatform => "service_infrastructure_platform",
            Self::CuratedResource => "curated_resource",
            Self::ProtocolSpecificationStandard => "protocol_specification_standard",
            Self::DatasetModelResearchArtifact => "dataset_model_research_artifact",
            Self::EducationalExampleTemplate => "educational_example_template",
            Self::ExperimentalProofOfConcept => "experimental_proof_of_concept",
        }
    }
}

/// Classified project maturity, matching the public evaluation contract.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ProjectMaturity {
    Concept,
    Prototype,
    Alpha,
    Beta,
    Stable,
    Maintenance,
    Dormant,
    Archived,
}

impl ProjectMaturity {
    const fn code(self) -> &'static str {
        match self {
            Self::Concept => "concept",
            Self::Prototype => "prototype",
            Self::Alpha => "alpha",
            Self::Beta => "beta",
            Self::Stable => "stable",
            Self::Maintenance => "maintenance",
            Self::Dormant => "dormant",
            Self::Archived => "archived",
        }
    }
}

/// Credential-independent provider identity recorded on the evaluation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum EvaluatorProvider {
    Deterministic,
    OpenaiApi,
    CodexCli,
    CodexOauth,
}

impl EvaluatorProvider {
    const fn code(self) -> &'static str {
        match self {
            Self::Deterministic => "deterministic",
            Self::OpenaiApi => "openai_api",
            Self::CodexCli => "codex_cli",
            Self::CodexOauth => "codex_oauth",
        }
    }
}

/// Publication scope recorded on the evaluation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Visibility {
    Public,
    PrivatePreview,
    PrivateLocal,
}

impl Visibility {
    const fn code(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::PrivatePreview => "private_preview",
            Self::PrivateLocal => "private_local",
        }
    }
}

/// Stable, redacted score-compilation failure category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScoreCompileErrorKind {
    InvalidContribution,
    InvalidClassification,
    InvalidStatement,
    InvalidEvaluator,
    UnknownCriterionDimension,
    RubricVersionMismatch,
}

/// A redacted compilation failure that never echoes source or path material.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScoreCompileError {
    kind: ScoreCompileErrorKind,
}

impl ScoreCompileError {
    const fn new(kind: ScoreCompileErrorKind) -> Self {
        Self { kind }
    }

    /// Returns the stable non-sensitive failure category.
    pub const fn kind(&self) -> ScoreCompileErrorKind {
        self.kind
    }
}

impl fmt::Display for ScoreCompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "score compilation failed ({:?})", self.kind)
    }
}

impl Error for ScoreCompileError {}

fn is_version_identifier(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty() || bytes.len() > 100 {
        return false;
    }
    let boundary = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit();
    boundary(bytes[0])
        && boundary(bytes[bytes.len() - 1])
        && bytes
            .iter()
            .all(|byte| boundary(*byte) || matches!(byte, b'.' | b'_' | b'-'))
}

fn is_machine_code(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty() || bytes.len() > 100 || !bytes[0].is_ascii_lowercase() {
        return false;
    }
    let mut previous_separator = true;
    for &byte in bytes {
        if matches!(byte, b'.' | b'_' | b'-') {
            if previous_separator {
                return false;
            }
            previous_separator = true;
        } else if byte.is_ascii_lowercase() || byte.is_ascii_digit() {
            previous_separator = false;
        } else {
            return false;
        }
    }
    !previous_separator
}

fn is_statement(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_STATEMENT_BYTES && !value.chars().any(char::is_control)
}

fn sorted_unique(mut ids: Vec<EvidenceId>) -> Vec<EvidenceId> {
    ids.sort();
    ids.dedup();
    ids
}

/// One deterministic rule contribution to a single dimension.
///
/// The optional value is a normalized `0.0..=1.0` sub-score and is present for
/// every applicability except `not_applicable`, which is an explicit exclusion
/// rather than a zero contribution.
#[derive(Clone, Debug, PartialEq)]
pub struct DeterministicContribution {
    rule_id: String,
    dimension: ScoreDimension,
    applicability: RubricApplicability,
    value: Option<f64>,
    confidence: f64,
    evidence_ids: Vec<EvidenceId>,
}

impl DeterministicContribution {
    /// Validates one deterministic rule contribution.
    pub fn new(
        rule_id: &str,
        dimension: ScoreDimension,
        applicability: RubricApplicability,
        value: Option<f64>,
        confidence: f64,
        evidence_ids: Vec<EvidenceId>,
    ) -> Result<Self, ScoreCompileError> {
        if !is_machine_code(rule_id) {
            return Err(ScoreCompileError::new(
                ScoreCompileErrorKind::InvalidContribution,
            ));
        }
        validate_normalized(applicability, value, confidence, &evidence_ids)
            .map_err(|()| ScoreCompileError::new(ScoreCompileErrorKind::InvalidContribution))?;
        Ok(Self {
            rule_id: rule_id.to_owned(),
            dimension,
            applicability,
            value,
            confidence,
            evidence_ids: sorted_unique(evidence_ids),
        })
    }

    /// Returns the versioned rule identifier.
    pub fn rule_id(&self) -> &str {
        &self.rule_id
    }

    /// Returns the dimension this contribution scores.
    pub const fn dimension(&self) -> ScoreDimension {
        self.dimension
    }
}

fn validate_normalized(
    applicability: RubricApplicability,
    value: Option<f64>,
    confidence: f64,
    evidence_ids: &[EvidenceId],
) -> Result<(), ()> {
    if !confidence.is_finite() || !(0.0..=1.0).contains(&confidence) {
        return Err(());
    }
    match (applicability, value) {
        (RubricApplicability::NotApplicable, Some(_)) => return Err(()),
        (RubricApplicability::NotApplicable, None) => {}
        (_, None) => return Err(()),
        (_, Some(value)) if !value.is_finite() || !(0.0..=1.0).contains(&value) => return Err(()),
        (_, Some(_)) => {}
    }
    if applicability != RubricApplicability::NotApplicable && evidence_ids.is_empty() {
        return Err(());
    }
    Ok(())
}

/// A classification supplied to the compiler by an upstream classifier stage.
///
/// The compiler consumes resolved applicability; it does not itself classify.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectClassification {
    status: EvidenceStatus,
    primary_type: Option<ProjectType>,
    secondary_types: Vec<ProjectType>,
    tags: Vec<String>,
    maturity: Option<ProjectMaturity>,
    confidence: f64,
    evidence_ids: Vec<EvidenceId>,
}

impl ProjectClassification {
    /// Validates a classification whose type and maturity presence match status.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        status: EvidenceStatus,
        primary_type: Option<ProjectType>,
        secondary_types: Vec<ProjectType>,
        tags: Vec<String>,
        maturity: Option<ProjectMaturity>,
        confidence: f64,
        evidence_ids: Vec<EvidenceId>,
    ) -> Result<Self, ScoreCompileError> {
        let invalid = |()| ScoreCompileError::new(ScoreCompileErrorKind::InvalidClassification);
        if !confidence.is_finite() || !(0.0..=1.0).contains(&confidence) {
            return Err(invalid(()));
        }
        if !tags.iter().all(|tag| is_machine_code(tag)) {
            return Err(invalid(()));
        }
        let usable = matches!(status, EvidenceStatus::Complete | EvidenceStatus::Partial);
        if usable != (primary_type.is_some() && maturity.is_some()) {
            return Err(invalid(()));
        }
        if usable && evidence_ids.is_empty() {
            return Err(invalid(()));
        }
        Ok(Self {
            status,
            primary_type,
            secondary_types,
            tags,
            maturity,
            confidence,
            evidence_ids: sorted_unique(evidence_ids),
        })
    }

    /// Returns cited classification evidence in canonical order.
    pub fn evidence_ids(&self) -> &[EvidenceId] {
        &self.evidence_ids
    }

    fn to_value(&self) -> Value {
        json!({
            "status": status_code(self.status),
            "primary_type": self.primary_type.map(ProjectType::code),
            "secondary_types": self.secondary_types.iter().map(|value| value.code()).collect::<Vec<_>>(),
            "tags": self.tags,
            "maturity": self.maturity.map(ProjectMaturity::code),
            "confidence": self.confidence,
            "evidence_ids": evidence_values(&self.evidence_ids),
        })
    }
}

/// A cited factual assumption or counter-signal statement.
#[derive(Clone, Debug, PartialEq)]
pub struct CitedStatement {
    text: String,
    evidence_ids: Vec<EvidenceId>,
}

impl CitedStatement {
    /// Validates a bounded statement that cites at least one evidence identifier.
    pub fn new(text: &str, evidence_ids: Vec<EvidenceId>) -> Result<Self, ScoreCompileError> {
        if !is_statement(text) || evidence_ids.is_empty() {
            return Err(ScoreCompileError::new(
                ScoreCompileErrorKind::InvalidStatement,
            ));
        }
        Ok(Self {
            text: text.to_owned(),
            evidence_ids: sorted_unique(evidence_ids),
        })
    }

    fn to_value(&self) -> Value {
        json!({ "text": self.text, "evidence_ids": evidence_values(&self.evidence_ids) })
    }
}

/// Separately supplied cited context for the Potential forecast.
///
/// The compiler validates citations and passes the narrative through; it does
/// not invent Potential prose.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PotentialContext {
    assumptions: Vec<CitedStatement>,
    major_counter_signals: Vec<CitedStatement>,
}

impl PotentialContext {
    /// Creates cited Potential context.
    pub fn new(
        assumptions: Vec<CitedStatement>,
        major_counter_signals: Vec<CitedStatement>,
    ) -> Self {
        Self {
            assumptions,
            major_counter_signals,
        }
    }
}

/// Provider-independent evaluator provenance recorded on the evaluation.
#[derive(Clone, Debug, PartialEq)]
pub struct EvaluatorDescriptor {
    profile: String,
    provider: EvaluatorProvider,
    model: Option<String>,
    rubric_version: String,
}

impl EvaluatorDescriptor {
    /// Validates evaluator provenance identifiers.
    pub fn new(
        profile: &str,
        provider: EvaluatorProvider,
        model: Option<&str>,
        rubric_version: &str,
    ) -> Result<Self, ScoreCompileError> {
        let invalid = ScoreCompileError::new(ScoreCompileErrorKind::InvalidEvaluator);
        if !is_version_identifier(profile) || !is_version_identifier(rubric_version) {
            return Err(invalid);
        }
        if let Some(model) = model
            && (model.is_empty() || model.len() > 200 || model.chars().any(char::is_control))
        {
            return Err(invalid);
        }
        Ok(Self {
            profile: profile.to_owned(),
            provider,
            model: model.map(str::to_owned),
            rubric_version: rubric_version.to_owned(),
        })
    }

    fn to_value(&self) -> Value {
        json!({
            "profile": self.profile,
            "provider": self.provider.code(),
            "model": self.model,
            "rubric_version": self.rubric_version,
        })
    }
}

/// Versioned weight, sufficiency, applicability, and forecast policy.
///
/// Every field is versioned data folded into the published rule-set hash, so a
/// weight or rule change is visible rather than a silent constant edit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerPolicy {
    compiler_version: &'static str,
    score_version: &'static str,
    potential_version: &'static str,
    weight_policy_version: &'static str,
    sufficiency_policy_version: &'static str,
    applicability_policy_version: &'static str,
    forecast_policy_version: &'static str,
    forecast_horizon: &'static str,
    partial_weight_basis_points: u32,
    provisional_confidence_basis_points: u32,
}

const DIMENSION_WEIGHTS: [(ScoreDimension, u32); 5] = [
    (ScoreDimension::ProjectSubstance, 25),
    (ScoreDimension::Originality, 20),
    (ScoreDimension::EngineeringRigor, 25),
    (ScoreDimension::OpenSourceReadiness, 15),
    (ScoreDimension::MaintenanceHealth, 15),
];

const ESSENTIAL_DIMENSIONS: [ScoreDimension; 3] = [
    ScoreDimension::ProjectSubstance,
    ScoreDimension::EngineeringRigor,
    ScoreDimension::OpenSourceReadiness,
];

impl CompilerPolicy {
    /// Returns the initial versioned compiler policy from the specification.
    pub const fn v1() -> Self {
        Self {
            compiler_version: "project-score-compiler-1",
            score_version: "project-score-1",
            potential_version: "potential-1",
            weight_policy_version: "project-score-weights-1",
            sufficiency_policy_version: "project-score-sufficiency-1",
            applicability_policy_version: "project-score-applicability-1",
            forecast_policy_version: "project-potential-forecast-1",
            forecast_horizon: "P1Y",
            partial_weight_basis_points: 5_000,
            provisional_confidence_basis_points: 6_000,
        }
    }

    /// Returns the compiler version recorded in the result.
    pub const fn compiler_version(&self) -> &'static str {
        self.compiler_version
    }

    fn weight(&self, dimension: ScoreDimension) -> f64 {
        DIMENSION_WEIGHTS
            .into_iter()
            .find_map(|(candidate, weight)| (candidate == dimension).then_some(f64::from(weight)))
            .unwrap_or(0.0)
    }

    fn is_essential(&self, dimension: ScoreDimension) -> bool {
        ESSENTIAL_DIMENSIONS.contains(&dimension)
    }

    fn partial_weight(&self) -> f64 {
        f64::from(self.partial_weight_basis_points) / 10_000.0
    }

    fn provisional_penalty(&self) -> f64 {
        f64::from(self.provisional_confidence_basis_points) / 10_000.0
    }

    fn rule_set_hash(&self) -> String {
        let mut hash = Sha256::new();
        let mut field = |value: &[u8]| {
            hash.update((value.len() as u64).to_be_bytes());
            hash.update(value);
        };
        field(RULE_SET_DOMAIN);
        for value in [
            self.compiler_version,
            self.score_version,
            self.potential_version,
            EVALUATION_VERSION,
            self.weight_policy_version,
            self.sufficiency_policy_version,
            self.applicability_policy_version,
            self.forecast_policy_version,
            self.forecast_horizon,
        ] {
            field(value.as_bytes());
        }
        for (dimension, weight) in DIMENSION_WEIGHTS {
            field(dimension.field_name().as_bytes());
            field(&(u64::from(weight)).to_be_bytes());
        }
        for dimension in ESSENTIAL_DIMENSIONS {
            field(dimension.field_name().as_bytes());
        }
        for value in [
            self.partial_weight_basis_points,
            self.provisional_confidence_basis_points,
        ] {
            field(&(u64::from(value)).to_be_bytes());
        }
        format!("sha256:{}", hex::encode(hash.finalize()))
    }
}

/// Where one score contribution originated, for rule-and-evidence explainability.
#[derive(Clone, Debug, PartialEq)]
pub enum ContributionSource {
    DeterministicRule(String),
    RubricCriterion(RubricCriterionId),
}

/// One explainable contribution to a dimension score.
#[derive(Clone, Debug, PartialEq)]
pub struct ScoreContribution {
    source: ContributionSource,
    applicability: RubricApplicability,
    normalized_value: Option<f64>,
    confidence: f64,
    evidence_ids: Vec<EvidenceId>,
}

impl ScoreContribution {
    /// Returns the originating rule or criterion.
    pub const fn source(&self) -> &ContributionSource {
        &self.source
    }

    /// Returns the contribution applicability.
    pub const fn applicability(&self) -> RubricApplicability {
        self.applicability
    }

    /// Returns the normalized sub-score, absent only when not applicable.
    pub const fn normalized_value(&self) -> Option<f64> {
        self.normalized_value
    }

    /// Returns cited evidence for this contribution.
    pub fn evidence_ids(&self) -> &[EvidenceId] {
        &self.evidence_ids
    }
}

/// One compiled dimension score with its contribution breakdown.
#[derive(Clone, Debug, PartialEq)]
pub struct DimensionScore {
    dimension: ScoreDimension,
    status: EvidenceStatus,
    value: Option<f64>,
    confidence: f64,
    version: String,
    evidence_ids: Vec<EvidenceId>,
    contributions: Vec<ScoreContribution>,
}

impl DimensionScore {
    /// Returns the scored dimension.
    pub const fn dimension(&self) -> ScoreDimension {
        self.dimension
    }

    /// Returns availability; unavailable and insufficient are never zero scores.
    pub const fn status(&self) -> EvidenceStatus {
        self.status
    }

    /// Returns the `0..=100` value, absent when not scoreable.
    pub const fn value(&self) -> Option<f64> {
        self.value
    }

    /// Returns score confidence in the closed unit interval.
    pub const fn confidence(&self) -> f64 {
        self.confidence
    }

    /// Returns the explainable rule and criterion contributions.
    pub fn contributions(&self) -> &[ScoreContribution] {
        &self.contributions
    }

    fn to_value(&self) -> Value {
        score_value(
            self.status,
            self.value,
            self.confidence,
            &self.version,
            &self.evidence_ids,
        )
    }
}

/// The overall Assay Score, compiled from available dimensions only.
#[derive(Clone, Debug, PartialEq)]
pub struct AssayScore {
    status: EvidenceStatus,
    value: Option<f64>,
    confidence: f64,
    provisional: bool,
    version: String,
    evidence_ids: Vec<EvidenceId>,
}

impl AssayScore {
    /// Returns availability; a missing essential dimension keeps it unscored.
    pub const fn status(&self) -> EvidenceStatus {
        self.status
    }

    /// Returns the `0..=100` value, absent when sufficiency is not met.
    pub const fn value(&self) -> Option<f64> {
        self.value
    }

    /// Returns whether the value is a low-confidence provisional normalization.
    pub const fn provisional(&self) -> bool {
        self.provisional
    }

    /// Returns overall confidence in the closed unit interval.
    pub const fn confidence(&self) -> f64 {
        self.confidence
    }

    fn to_value(&self) -> Value {
        score_value(
            self.status,
            self.value,
            self.confidence,
            &self.version,
            &self.evidence_ids,
        )
    }
}

/// The separate Potential indicator, never included in the Assay Score.
#[derive(Clone, Debug, PartialEq)]
pub struct PotentialScore {
    status: EvidenceStatus,
    value: Option<f64>,
    confidence: f64,
    version: String,
    evidence_ids: Vec<EvidenceId>,
    forecast_horizon: String,
    assumptions: Vec<CitedStatement>,
    major_counter_signals: Vec<CitedStatement>,
    contributions: Vec<ScoreContribution>,
}

impl PotentialScore {
    /// Returns Potential availability.
    pub const fn status(&self) -> EvidenceStatus {
        self.status
    }

    /// Returns the `0..=100` Potential value, absent when not forecastable.
    pub const fn value(&self) -> Option<f64> {
        self.value
    }

    fn to_value(&self) -> Value {
        json!({
            "status": status_code(self.status),
            "value": self.value,
            "confidence": self.confidence,
            "version": self.version,
            "evidence_ids": evidence_values(&self.evidence_ids),
            "forecast_horizon": self.forecast_horizon,
            "assumptions": self.assumptions.iter().map(CitedStatement::to_value).collect::<Vec<_>>(),
            "major_counter_signals": self.major_counter_signals.iter().map(CitedStatement::to_value).collect::<Vec<_>>(),
        })
    }
}

/// All inputs required to compile one project evaluation.
pub struct ScoreCompilerInput {
    project_source: RepositorySource,
    revision: RevisionId,
    evaluator: EvaluatorDescriptor,
    visibility: Visibility,
    classification: ProjectClassification,
    deterministic: Vec<DeterministicContribution>,
    judgments: Option<RubricJudgmentSet>,
    potential_context: PotentialContext,
    policy: CompilerPolicy,
}

impl ScoreCompilerInput {
    /// Gathers pre-validated inputs for one deterministic compilation.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        project_source: RepositorySource,
        revision: RevisionId,
        evaluator: EvaluatorDescriptor,
        visibility: Visibility,
        classification: ProjectClassification,
        deterministic: Vec<DeterministicContribution>,
        judgments: Option<RubricJudgmentSet>,
        potential_context: PotentialContext,
        policy: CompilerPolicy,
    ) -> Self {
        Self {
            project_source,
            revision,
            evaluator,
            visibility,
            classification,
            deterministic,
            judgments,
            potential_context,
            policy,
        }
    }

    /// Compiles the deterministic, versioned project evaluation.
    pub fn compile(&self) -> Result<CompiledEvaluation, ScoreCompileError> {
        let mut grouped: BTreeMap<ScoreDimension, Vec<ScoreContribution>> = BTreeMap::new();
        for contribution in &self.deterministic {
            grouped
                .entry(contribution.dimension)
                .or_default()
                .push(ScoreContribution {
                    source: ContributionSource::DeterministicRule(contribution.rule_id.clone()),
                    applicability: contribution.applicability,
                    normalized_value: contribution.value,
                    confidence: contribution.confidence,
                    evidence_ids: contribution.evidence_ids.clone(),
                });
        }

        let judgment_bundle_hash = match &self.judgments {
            Some(set) => {
                if set.evaluation_version().as_str() != EVALUATION_VERSION
                    || set.rubric_version().as_str() != self.evaluator.rubric_version
                {
                    return Err(ScoreCompileError::new(
                        ScoreCompileErrorKind::RubricVersionMismatch,
                    ));
                }
                for judgment in set.judgments() {
                    let dimension = ScoreDimension::from_criterion_prefix(
                        judgment.criterion_id().dimension_prefix(),
                    )
                    .ok_or_else(|| {
                        ScoreCompileError::new(ScoreCompileErrorKind::UnknownCriterionDimension)
                    })?;
                    let normalized = judgment
                        .rating()
                        .map(|rating| f64::from(rating) / f64::from(judgment.rating_scale()));
                    grouped
                        .entry(dimension)
                        .or_default()
                        .push(ScoreContribution {
                            source: ContributionSource::RubricCriterion(
                                judgment.criterion_id().clone(),
                            ),
                            applicability: judgment.applicability(),
                            normalized_value: normalized,
                            confidence: judgment.confidence(),
                            evidence_ids: judgment.evidence_ids().to_vec(),
                        });
                }
                Some(set.evidence_bundle_hash().as_str().to_owned())
            }
            None => None,
        };

        let mut dimensions = BTreeMap::new();
        for dimension in ASSAY_SCORE_DIMENSIONS {
            let score = self.compile_dimension(
                dimension,
                grouped.remove(&dimension).unwrap_or_default(),
                self.policy.score_version,
            );
            dimensions.insert(dimension, score);
        }

        let assay_score = self.compile_assay_score(&dimensions);
        let potential = self.compile_potential(
            grouped
                .remove(&ScoreDimension::Potential)
                .unwrap_or_default(),
        );

        let run_status = if assay_score.status == EvidenceStatus::Complete {
            EvidenceStatus::Complete
        } else {
            EvidenceStatus::Partial
        };

        let warnings = self.build_warnings(&assay_score);
        let limitations = self.build_limitations(&assay_score, &dimensions);
        let evidence_ids =
            self.collect_evidence(&dimensions, &assay_score, &potential, &limitations);

        Ok(CompiledEvaluation {
            status: run_status,
            provisional: assay_score.provisional,
            visibility: self.visibility,
            evaluator: self.evaluator.clone(),
            compiler_version: self.policy.compiler_version,
            rule_set_hash: self.policy.rule_set_hash(),
            judgment_bundle_hash,
            project_source: self.project_source.clone(),
            revision: self.revision.clone(),
            classification: self.classification.clone(),
            assay_score,
            dimensions,
            potential,
            evidence_ids,
            warnings,
            limitations,
        })
    }

    fn compile_dimension(
        &self,
        dimension: ScoreDimension,
        contributions: Vec<ScoreContribution>,
        version: &str,
    ) -> DimensionScore {
        let evidence_ids = sorted_unique(
            contributions
                .iter()
                .flat_map(|contribution| contribution.evidence_ids.iter().cloned())
                .collect(),
        );
        let scoreable = contributions
            .iter()
            .filter(|contribution| {
                contribution.applicability != RubricApplicability::NotApplicable
                    && contribution.normalized_value.is_some()
            })
            .collect::<Vec<_>>();

        if scoreable.is_empty() {
            let status = if self.policy.is_essential(dimension) {
                EvidenceStatus::Insufficient
            } else {
                EvidenceStatus::Unavailable
            };
            return DimensionScore {
                dimension,
                status,
                value: None,
                confidence: 0.0,
                version: version.to_owned(),
                evidence_ids,
                contributions,
            };
        }

        let mut weight_sum = 0.0;
        let mut value_sum = 0.0;
        let mut confidence_sum = 0.0;
        for contribution in &scoreable {
            let weight = match contribution.applicability {
                RubricApplicability::Applicable => 1.0,
                RubricApplicability::PartiallyApplicable => self.policy.partial_weight(),
                RubricApplicability::NotApplicable => continue,
            };
            weight_sum += weight;
            value_sum += weight * contribution.normalized_value.unwrap_or(0.0);
            confidence_sum += weight * contribution.confidence;
        }
        // The input contract gives every non-not_applicable contribution a
        // value, so a scoreable dimension has no per-contribution gap state.
        DimensionScore {
            dimension,
            status: EvidenceStatus::Complete,
            value: Some(value_sum / weight_sum * 100.0),
            confidence: confidence_sum / weight_sum,
            version: version.to_owned(),
            evidence_ids,
            contributions,
        }
    }

    fn compile_assay_score(
        &self,
        dimensions: &BTreeMap<ScoreDimension, DimensionScore>,
    ) -> AssayScore {
        let available = ASSAY_SCORE_DIMENSIONS
            .into_iter()
            .filter(|dimension| dimensions[dimension].value.is_some())
            .collect::<Vec<_>>();
        let essential_available = ESSENTIAL_DIMENSIONS
            .into_iter()
            .all(|dimension| dimensions[&dimension].value.is_some());
        let version = self.policy.score_version.to_owned();

        if !essential_available {
            let any_insufficient = ESSENTIAL_DIMENSIONS
                .into_iter()
                .any(|dimension| dimensions[&dimension].status == EvidenceStatus::Insufficient);
            let status = if any_insufficient {
                EvidenceStatus::Insufficient
            } else {
                EvidenceStatus::Unavailable
            };
            return AssayScore {
                status,
                value: None,
                confidence: 0.0,
                provisional: false,
                version,
                evidence_ids: Vec::new(),
            };
        }

        let mut weight_sum = 0.0;
        let mut value_sum = 0.0;
        let mut confidence_sum = 0.0;
        for dimension in &available {
            let score = &dimensions[dimension];
            let weight = self.policy.weight(*dimension);
            weight_sum += weight;
            value_sum += weight * score.value.unwrap_or(0.0);
            confidence_sum += weight * score.confidence;
        }
        let provisional = available.len() != ASSAY_SCORE_DIMENSIONS.len();
        let mut confidence = confidence_sum / weight_sum;
        if provisional {
            confidence *= self.policy.provisional_penalty();
        }
        let status = if provisional {
            EvidenceStatus::Partial
        } else {
            EvidenceStatus::Complete
        };
        let evidence_ids = sorted_unique(
            available
                .iter()
                .flat_map(|dimension| dimensions[dimension].evidence_ids.iter().cloned())
                .collect(),
        );
        AssayScore {
            status,
            value: Some(value_sum / weight_sum),
            confidence,
            provisional,
            version,
            evidence_ids,
        }
    }

    fn compile_potential(&self, contributions: Vec<ScoreContribution>) -> PotentialScore {
        let base = self.compile_dimension(
            ScoreDimension::Potential,
            contributions,
            self.policy.potential_version,
        );
        PotentialScore {
            status: base.status,
            value: base.value,
            confidence: base.confidence,
            version: self.policy.potential_version.to_owned(),
            evidence_ids: base.evidence_ids,
            forecast_horizon: self.policy.forecast_horizon.to_owned(),
            assumptions: self.potential_context.assumptions.clone(),
            major_counter_signals: self.potential_context.major_counter_signals.clone(),
            contributions: base.contributions,
        }
    }

    fn build_warnings(&self, assay_score: &AssayScore) -> Vec<(String, Vec<EvidenceId>)> {
        let mut warnings = Vec::new();
        if assay_score.status != EvidenceStatus::Complete {
            warnings.push(("score_release_gate_not_met".to_owned(), Vec::new()));
        }
        warnings
    }

    fn build_limitations(
        &self,
        assay_score: &AssayScore,
        dimensions: &BTreeMap<ScoreDimension, DimensionScore>,
    ) -> Vec<(String, Vec<EvidenceId>)> {
        let mut limitations = vec![(
            "repository_code_not_executed".to_owned(),
            self.classification.evidence_ids.clone(),
        )];
        if assay_score.provisional {
            limitations.push(("provisional_score_normalization".to_owned(), Vec::new()));
            let missing = sorted_unique(
                ASSAY_SCORE_DIMENSIONS
                    .into_iter()
                    .filter(|dimension| dimensions[dimension].value.is_none())
                    .flat_map(|dimension| dimensions[&dimension].evidence_ids.clone())
                    .collect(),
            );
            limitations.push(("missing_dimension_evidence".to_owned(), missing));
        }
        limitations.sort_by(|left, right| left.0.cmp(&right.0));
        limitations
    }

    fn collect_evidence(
        &self,
        dimensions: &BTreeMap<ScoreDimension, DimensionScore>,
        assay_score: &AssayScore,
        potential: &PotentialScore,
        limitations: &[(String, Vec<EvidenceId>)],
    ) -> Vec<EvidenceId> {
        let mut ids = self.classification.evidence_ids.clone();
        ids.extend(assay_score.evidence_ids.iter().cloned());
        for score in dimensions.values() {
            ids.extend(score.evidence_ids.iter().cloned());
        }
        ids.extend(potential.evidence_ids.iter().cloned());
        for statement in potential
            .assumptions
            .iter()
            .chain(&potential.major_counter_signals)
        {
            ids.extend(statement.evidence_ids.iter().cloned());
        }
        for (_, evidence) in limitations {
            ids.extend(evidence.iter().cloned());
        }
        sorted_unique(ids)
    }
}

/// A compiled, dimensioned project evaluation with a public machine mapping.
#[derive(Clone, Debug, PartialEq)]
pub struct CompiledEvaluation {
    status: EvidenceStatus,
    provisional: bool,
    visibility: Visibility,
    evaluator: EvaluatorDescriptor,
    compiler_version: &'static str,
    rule_set_hash: String,
    judgment_bundle_hash: Option<String>,
    project_source: RepositorySource,
    revision: RevisionId,
    classification: ProjectClassification,
    assay_score: AssayScore,
    dimensions: BTreeMap<ScoreDimension, DimensionScore>,
    potential: PotentialScore,
    evidence_ids: Vec<EvidenceId>,
    warnings: Vec<(String, Vec<EvidenceId>)>,
    limitations: Vec<(String, Vec<EvidenceId>)>,
}

impl CompiledEvaluation {
    /// Returns the overall Assay Score.
    pub const fn assay_score(&self) -> &AssayScore {
        &self.assay_score
    }

    /// Returns one compiled Assay Score dimension.
    pub fn dimension(&self, dimension: ScoreDimension) -> Option<&DimensionScore> {
        self.dimensions.get(&dimension)
    }

    /// Returns the separate Potential indicator.
    pub const fn potential(&self) -> &PotentialScore {
        &self.potential
    }

    /// Returns whether the Assay Score is a provisional normalization.
    pub const fn provisional(&self) -> bool {
        self.provisional
    }

    /// Maps the evaluation onto `schemas/project-evaluation/v1.json`.
    pub fn to_machine_value(&self) -> Value {
        let scores = json!({
            "assay_score": self.assay_score.to_value(),
            "project_substance": self.dimensions[&ScoreDimension::ProjectSubstance].to_value(),
            "originality": self.dimensions[&ScoreDimension::Originality].to_value(),
            "engineering_rigor": self.dimensions[&ScoreDimension::EngineeringRigor].to_value(),
            "open_source_readiness": self.dimensions[&ScoreDimension::OpenSourceReadiness].to_value(),
            "maintenance_health": self.dimensions[&ScoreDimension::MaintenanceHealth].to_value(),
            "potential": self.potential.to_value(),
        });
        json!({
            "schema_version": SCHEMA_VERSION,
            "evaluation_version": EVALUATION_VERSION,
            "status": status_code(self.status),
            "provisional": self.provisional,
            "visibility": self.visibility.code(),
            "evaluator": self.evaluator.to_value(),
            "compiler": {
                "version": self.compiler_version,
                "rule_set_hash": self.rule_set_hash,
                "judgment_bundle_hash": self.judgment_bundle_hash,
            },
            "project": {
                "source": repository_value(&self.project_source),
                "revision": self.revision.as_str(),
            },
            "classification": self.classification.to_value(),
            "scores": scores,
            "evidence_ids": evidence_values(&self.evidence_ids),
            "introduction": {
                "status": status_code(EvidenceStatus::Unavailable),
                "factual_statements": [],
                "interpretations": [],
            },
            "warnings": diagnostics(&self.warnings),
            "limitations": diagnostics(&self.limitations),
        })
    }
}

fn score_value(
    status: EvidenceStatus,
    value: Option<f64>,
    confidence: f64,
    version: &str,
    evidence_ids: &[EvidenceId],
) -> Value {
    json!({
        "status": status_code(status),
        "value": value,
        "confidence": confidence,
        "version": version,
        "evidence_ids": evidence_values(evidence_ids),
    })
}

fn diagnostics(entries: &[(String, Vec<EvidenceId>)]) -> Value {
    Value::Array(
        entries
            .iter()
            .map(|(code, evidence_ids)| {
                json!({ "code": code, "evidence_ids": evidence_values(evidence_ids) })
            })
            .collect(),
    )
}

fn evidence_values(evidence_ids: &[EvidenceId]) -> Vec<&str> {
    evidence_ids.iter().map(EvidenceId::as_str).collect()
}

fn repository_value(source: &RepositorySource) -> Value {
    if let Some(id) = source.local_repository_id() {
        json!({ "kind": "local", "repository_id": id.as_str() })
    } else if let Some((provider, namespace, repository)) = source.hosted_locator() {
        json!({ "kind": "hosted", "provider": provider, "namespace": namespace, "repository": repository })
    } else {
        unreachable!("repository source variants are closed")
    }
}

const fn status_code(status: EvidenceStatus) -> &'static str {
    match status {
        EvidenceStatus::Complete => "complete",
        EvidenceStatus::Partial => "partial",
        EvidenceStatus::Unavailable => "unavailable",
        EvidenceStatus::Unsupported => "unsupported",
        EvidenceStatus::Insufficient => "insufficient",
        EvidenceStatus::Pending => "pending",
    }
}
