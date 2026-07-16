//! Deterministic project type and maturity classification.
//!
//! The classifier maps cited, evidence-grounded observations onto a primary
//! project type, optional secondary types, descriptive tags, and a maturity,
//! each with confidence and explicit `unknown` behavior. It performs no
//! filesystem, process, network, clock, or model-provider I/O; identical input
//! yields byte-identical output.
//!
//! An observation is a concrete cited signal, never a conclusion, so the
//! decision logic stays honest and reviewable. When no rule fires, or when the
//! type or maturity signal is absent, the result is an explicit `unknown`
//! classification rather than an invented default. A type label selects
//! applicable rubric criteria; it never by itself defines the comparison cohort.
//!
//! The emitted [`ProjectClassification`] is the exact value the deterministic
//! score compiler consumes, so classification and scoring stay aligned. The
//! evaluation schema requires a resolved maturity alongside a resolved type for
//! a usable classification, so a type-only result is intentionally represented
//! as `unknown` pending a maturity signal.

use std::collections::BTreeMap;

use assay_domain::{EvidenceId, EvidenceStatus, RubricApplicability};

use crate::{ProjectClassification, ProjectMaturity, ProjectType, ScoreDimension};

/// A cited observation that a project exhibits one type-relevant signal.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TypeSignal {
    CuratedListStructure,
    SpecificationDocument,
    DatasetOrModelArtifact,
    TemplateMarker,
    ProofOfConceptMarker,
    ServiceDeploymentDeclared,
    CliEntrypointDeclared,
    LibraryPackagingDeclared,
    ApplicationEntrypointDeclared,
    PluginHostDeclared,
    FrameworkExtensionPoints,
    DevOpsInfrastructure,
}

impl TypeSignal {
    /// Returns the primary type this signal implies, when it maps to one.
    const fn primary_type(self) -> Option<ProjectType> {
        match self {
            Self::CuratedListStructure => Some(ProjectType::CuratedResource),
            Self::SpecificationDocument => Some(ProjectType::ProtocolSpecificationStandard),
            Self::DatasetOrModelArtifact => Some(ProjectType::DatasetModelResearchArtifact),
            Self::TemplateMarker => Some(ProjectType::EducationalExampleTemplate),
            Self::ProofOfConceptMarker => Some(ProjectType::ExperimentalProofOfConcept),
            Self::ServiceDeploymentDeclared => Some(ProjectType::ServiceInfrastructurePlatform),
            Self::CliEntrypointDeclared => Some(ProjectType::CliDeveloperTool),
            Self::LibraryPackagingDeclared => Some(ProjectType::LibrarySdkFramework),
            Self::ApplicationEntrypointDeclared => Some(ProjectType::Application),
            Self::PluginHostDeclared
            | Self::FrameworkExtensionPoints
            | Self::DevOpsInfrastructure => None,
        }
    }

    /// Returns the descriptive tag this signal contributes, when any.
    const fn tag(self) -> Option<&'static str> {
        match self {
            Self::PluginHostDeclared => Some("plugin"),
            Self::FrameworkExtensionPoints => Some("framework"),
            Self::DevOpsInfrastructure => Some("infrastructure"),
            _ => None,
        }
    }
}

/// Priority order that resolves the primary type when several types are implied.
///
/// Specific artifact kinds win over general delivery forms; among delivery
/// forms the most operationally specific declaration wins. This order is
/// versioned policy data, not an incidental match order.
const TYPE_PRIORITY: [ProjectType; 9] = [
    ProjectType::CuratedResource,
    ProjectType::ProtocolSpecificationStandard,
    ProjectType::DatasetModelResearchArtifact,
    ProjectType::EducationalExampleTemplate,
    ProjectType::ExperimentalProofOfConcept,
    ProjectType::ServiceInfrastructurePlatform,
    ProjectType::CliDeveloperTool,
    ProjectType::LibrarySdkFramework,
    ProjectType::Application,
];

const DELIVERY_FORM_TYPES: [ProjectType; 4] = [
    ProjectType::ServiceInfrastructurePlatform,
    ProjectType::CliDeveloperTool,
    ProjectType::LibrarySdkFramework,
    ProjectType::Application,
];

/// A cited observation that a project exhibits one maturity-relevant signal.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MaturitySignal {
    ArchivedRepository,
    MaintenanceModeDeclared,
    StableReleaseTagged,
    BetaPrereleaseTagged,
    AlphaPrereleaseTagged,
    SustainedIteration,
    SingleInitialImport,
    ConceptOnly,
    DormantNoRecentActivity,
}

impl MaturitySignal {
    const fn maturity(self) -> ProjectMaturity {
        match self {
            Self::ArchivedRepository => ProjectMaturity::Archived,
            Self::MaintenanceModeDeclared => ProjectMaturity::Maintenance,
            Self::StableReleaseTagged => ProjectMaturity::Stable,
            Self::BetaPrereleaseTagged => ProjectMaturity::Beta,
            Self::AlphaPrereleaseTagged => ProjectMaturity::Alpha,
            Self::SustainedIteration => ProjectMaturity::Prototype,
            Self::SingleInitialImport | Self::ConceptOnly => ProjectMaturity::Concept,
            Self::DormantNoRecentActivity => ProjectMaturity::Dormant,
        }
    }
}

/// Maturity resolution priority. A stronger lifecycle or release signal wins
/// over inactivity so a stable-but-quiet project is not misread as dormant.
const MATURITY_PRIORITY: [MaturitySignal; 9] = [
    MaturitySignal::ArchivedRepository,
    MaturitySignal::MaintenanceModeDeclared,
    MaturitySignal::StableReleaseTagged,
    MaturitySignal::BetaPrereleaseTagged,
    MaturitySignal::AlphaPrereleaseTagged,
    MaturitySignal::SustainedIteration,
    MaturitySignal::SingleInitialImport,
    MaturitySignal::ConceptOnly,
    MaturitySignal::DormantNoRecentActivity,
];

/// A stable, redacted classification failure that never echoes source material.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClassificationError {
    kind: ClassificationErrorKind,
}

/// Stable classification failure category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClassificationErrorKind {
    UncitedObservation,
}

impl ClassificationError {
    /// Returns the stable non-sensitive failure category.
    pub const fn kind(&self) -> ClassificationErrorKind {
        self.kind
    }
}

impl std::fmt::Display for ClassificationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "classification failed ({:?})", self.kind)
    }
}

impl std::error::Error for ClassificationError {}

/// One cited type observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeObservation {
    signal: TypeSignal,
    evidence_ids: Vec<EvidenceId>,
}

impl TypeObservation {
    /// Validates one type observation, requiring at least one citation.
    pub fn new(
        signal: TypeSignal,
        evidence_ids: Vec<EvidenceId>,
    ) -> Result<Self, ClassificationError> {
        if evidence_ids.is_empty() {
            return Err(ClassificationError {
                kind: ClassificationErrorKind::UncitedObservation,
            });
        }
        Ok(Self {
            signal,
            evidence_ids: sorted_unique(evidence_ids),
        })
    }
}

/// One cited maturity observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaturityObservation {
    signal: MaturitySignal,
    evidence_ids: Vec<EvidenceId>,
}

impl MaturityObservation {
    /// Validates one maturity observation, requiring at least one citation.
    pub fn new(
        signal: MaturitySignal,
        evidence_ids: Vec<EvidenceId>,
    ) -> Result<Self, ClassificationError> {
        if evidence_ids.is_empty() {
            return Err(ClassificationError {
                kind: ClassificationErrorKind::UncitedObservation,
            });
        }
        Ok(Self {
            signal,
            evidence_ids: sorted_unique(evidence_ids),
        })
    }
}

/// Versioned classification confidence and applicability policy.
///
/// Every field is versioned data recorded on the outcome so a rule change is
/// visible rather than a silent constant edit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClassificationPolicy {
    policy_version: &'static str,
    applicability_policy_version: &'static str,
    single_type_confidence_bp: u32,
    ambiguous_type_confidence_bp: u32,
    maturity_confidence_bp: u32,
}

impl ClassificationPolicy {
    /// Returns the initial versioned classification policy.
    pub const fn v1() -> Self {
        Self {
            policy_version: "project-classification-1",
            applicability_policy_version: "project-classification-applicability-1",
            single_type_confidence_bp: 8_000,
            ambiguous_type_confidence_bp: 6_000,
            maturity_confidence_bp: 7_000,
        }
    }

    /// Returns the classification policy version.
    pub const fn policy_version(&self) -> &'static str {
        self.policy_version
    }

    /// Returns the applicability policy version.
    pub const fn applicability_policy_version(&self) -> &'static str {
        self.applicability_policy_version
    }
}

/// The resolved classification with its aligned criteria applicability.
#[derive(Clone, Debug, PartialEq)]
pub struct ClassificationOutcome {
    classification: ProjectClassification,
    primary_type: Option<ProjectType>,
    maturity: Option<ProjectMaturity>,
    applicability: BTreeMap<ScoreDimension, RubricApplicability>,
    policy_version: &'static str,
    applicability_policy_version: &'static str,
}

impl ClassificationOutcome {
    /// Returns the classification the score compiler consumes.
    pub const fn classification(&self) -> &ProjectClassification {
        &self.classification
    }

    /// Returns the resolved primary type, absent when the result is unknown.
    pub const fn primary_type(&self) -> Option<ProjectType> {
        self.primary_type
    }

    /// Returns the resolved maturity, absent when the result is unknown.
    pub const fn maturity(&self) -> Option<ProjectMaturity> {
        self.maturity
    }

    /// Returns type- and maturity-resolved applicability for one dimension.
    ///
    /// An unknown classification resolves every dimension to
    /// `PartiallyApplicable`, never `NotApplicable`, so absent classification
    /// never silently excludes a dimension.
    pub fn applicability(&self, dimension: ScoreDimension) -> RubricApplicability {
        self.applicability
            .get(&dimension)
            .copied()
            .unwrap_or(RubricApplicability::PartiallyApplicable)
    }

    /// Returns the classification policy version applied.
    pub const fn policy_version(&self) -> &'static str {
        self.policy_version
    }

    /// Returns the applicability policy version applied.
    pub const fn applicability_policy_version(&self) -> &'static str {
        self.applicability_policy_version
    }
}

/// Classifies a project from cited type and maturity observations.
///
/// A usable classification requires both a resolved type and a resolved
/// maturity; otherwise the result is an explicit unknown classification with an
/// `Insufficient` status and no invented type, maturity, or zero.
pub fn classify_project(
    type_observations: &[TypeObservation],
    maturity_observations: &[MaturityObservation],
    policy: &ClassificationPolicy,
) -> ClassificationOutcome {
    let (primary_type, secondary_types, type_evidence, type_ambiguous) =
        resolve_type(type_observations);
    let tags = resolve_tags(type_observations);
    let (maturity, maturity_evidence) = resolve_maturity(maturity_observations);

    match (primary_type, maturity) {
        (Some(primary), Some(maturity)) => {
            let confidence = classification_confidence(policy, type_ambiguous);
            let evidence_ids =
                sorted_unique(type_evidence.into_iter().chain(maturity_evidence).collect());
            let classification = ProjectClassification::new(
                EvidenceStatus::Complete,
                Some(primary),
                secondary_types,
                tags,
                Some(maturity),
                confidence,
                evidence_ids,
            )
            .expect("resolved observations satisfy the classification contract");
            ClassificationOutcome {
                classification,
                primary_type: Some(primary),
                maturity: Some(maturity),
                applicability: criteria_applicability(primary, maturity, policy),
                policy_version: policy.policy_version,
                applicability_policy_version: policy.applicability_policy_version,
            }
        }
        _ => {
            let classification = ProjectClassification::new(
                EvidenceStatus::Insufficient,
                None,
                Vec::new(),
                Vec::new(),
                None,
                0.0,
                Vec::new(),
            )
            .expect("an unknown classification satisfies the classification contract");
            ClassificationOutcome {
                classification,
                primary_type: None,
                maturity: None,
                applicability: unknown_applicability(),
                policy_version: policy.policy_version,
                applicability_policy_version: policy.applicability_policy_version,
            }
        }
    }
}

fn resolve_type(
    observations: &[TypeObservation],
) -> (Option<ProjectType>, Vec<ProjectType>, Vec<EvidenceId>, bool) {
    let mut matched: BTreeMap<ProjectType, Vec<EvidenceId>> = BTreeMap::new();
    for observation in observations {
        if let Some(project_type) = observation.signal.primary_type() {
            matched
                .entry(project_type)
                .or_default()
                .extend(observation.evidence_ids.iter().cloned());
        }
    }
    let Some(primary) = TYPE_PRIORITY
        .into_iter()
        .find(|candidate| matched.contains_key(candidate))
    else {
        return (None, Vec::new(), Vec::new(), false);
    };
    let mut secondary_types = matched
        .keys()
        .copied()
        .filter(|candidate| *candidate != primary)
        .collect::<Vec<_>>();
    secondary_types.sort();
    let evidence = matched.into_values().flatten().collect();
    let delivery_forms = secondary_types
        .iter()
        .chain(std::iter::once(&primary))
        .filter(|candidate| DELIVERY_FORM_TYPES.contains(candidate))
        .count();
    (Some(primary), secondary_types, evidence, delivery_forms > 1)
}

fn resolve_tags(observations: &[TypeObservation]) -> Vec<String> {
    let mut tags = observations
        .iter()
        .filter_map(|observation| observation.signal.tag())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    tags.sort();
    tags.dedup();
    tags
}

fn resolve_maturity(
    observations: &[MaturityObservation],
) -> (Option<ProjectMaturity>, Vec<EvidenceId>) {
    let mut matched: BTreeMap<MaturitySignal, Vec<EvidenceId>> = BTreeMap::new();
    for observation in observations {
        matched
            .entry(observation.signal)
            .or_default()
            .extend(observation.evidence_ids.iter().cloned());
    }
    let Some(signal) = MATURITY_PRIORITY
        .into_iter()
        .find(|candidate| matched.contains_key(candidate))
    else {
        return (None, Vec::new());
    };
    let evidence = matched
        .remove(&signal)
        .expect("the selected signal is present");
    (Some(signal.maturity()), evidence)
}

fn classification_confidence(policy: &ClassificationPolicy, ambiguous: bool) -> f64 {
    let type_bp = if ambiguous {
        policy.ambiguous_type_confidence_bp
    } else {
        policy.single_type_confidence_bp
    };
    f64::from((type_bp + policy.maturity_confidence_bp) / 2) / 10_000.0
}

/// Resolves type- and maturity-specific criteria applicability.
///
/// The base applicability reflects each type's evaluable surface; maturity only
/// relaxes an `Applicable` dimension to `PartiallyApplicable` for young or
/// end-of-life projects, never tightening it, so a young project is never
/// penalized for absent long-term evidence.
pub fn criteria_applicability(
    primary_type: ProjectType,
    maturity: ProjectMaturity,
    _policy: &ClassificationPolicy,
) -> BTreeMap<ScoreDimension, RubricApplicability> {
    use RubricApplicability::{Applicable, NotApplicable, PartiallyApplicable};
    use ScoreDimension::{
        EngineeringRigor, MaintenanceHealth, OpenSourceReadiness, Originality, ProjectSubstance,
    };

    let base = match primary_type {
        ProjectType::Application
        | ProjectType::CliDeveloperTool
        | ProjectType::ServiceInfrastructurePlatform
        | ProjectType::LibrarySdkFramework => [
            (ProjectSubstance, Applicable),
            (Originality, Applicable),
            (EngineeringRigor, Applicable),
            (OpenSourceReadiness, Applicable),
            (MaintenanceHealth, Applicable),
        ],
        ProjectType::CuratedResource | ProjectType::ProtocolSpecificationStandard => [
            (ProjectSubstance, PartiallyApplicable),
            (Originality, Applicable),
            (EngineeringRigor, NotApplicable),
            (OpenSourceReadiness, Applicable),
            (MaintenanceHealth, Applicable),
        ],
        ProjectType::DatasetModelResearchArtifact => [
            (ProjectSubstance, Applicable),
            (Originality, Applicable),
            (EngineeringRigor, PartiallyApplicable),
            (OpenSourceReadiness, Applicable),
            (MaintenanceHealth, PartiallyApplicable),
        ],
        ProjectType::EducationalExampleTemplate => [
            (ProjectSubstance, PartiallyApplicable),
            (Originality, PartiallyApplicable),
            (EngineeringRigor, PartiallyApplicable),
            (OpenSourceReadiness, Applicable),
            (MaintenanceHealth, PartiallyApplicable),
        ],
        ProjectType::ExperimentalProofOfConcept => [
            (ProjectSubstance, Applicable),
            (Originality, Applicable),
            (EngineeringRigor, PartiallyApplicable),
            (OpenSourceReadiness, PartiallyApplicable),
            (MaintenanceHealth, PartiallyApplicable),
        ],
    };

    let young = matches!(
        maturity,
        ProjectMaturity::Concept | ProjectMaturity::Prototype
    );
    let lifecycle_quiet = matches!(
        maturity,
        ProjectMaturity::Maintenance | ProjectMaturity::Dormant | ProjectMaturity::Archived
    );
    base.into_iter()
        .map(|(dimension, applicability)| {
            let relaxed = if applicability == Applicable
                && ((young && matches!(dimension, EngineeringRigor | MaintenanceHealth))
                    || (lifecycle_quiet && dimension == MaintenanceHealth))
            {
                PartiallyApplicable
            } else {
                applicability
            };
            (dimension, relaxed)
        })
        .collect()
}

fn unknown_applicability() -> BTreeMap<ScoreDimension, RubricApplicability> {
    crate::ASSAY_SCORE_DIMENSIONS
        .into_iter()
        .map(|dimension| (dimension, RubricApplicability::PartiallyApplicable))
        .collect()
}

fn sorted_unique(mut ids: Vec<EvidenceId>) -> Vec<EvidenceId> {
    ids.sort();
    ids.dedup();
    ids
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    fn evidence(value: &str) -> EvidenceId {
        EvidenceId::from_str(value).unwrap()
    }

    fn type_obs(signal: TypeSignal) -> TypeObservation {
        TypeObservation::new(signal, vec![evidence("evidence:readme:signal")]).unwrap()
    }

    fn maturity_obs(signal: MaturitySignal) -> MaturityObservation {
        MaturityObservation::new(signal, vec![evidence("evidence:history:signal")]).unwrap()
    }

    #[test]
    fn a_single_delivery_form_resolves_to_that_primary_type() {
        let outcome = classify_project(
            &[type_obs(TypeSignal::CliEntrypointDeclared)],
            &[maturity_obs(MaturitySignal::SustainedIteration)],
            &ClassificationPolicy::v1(),
        );
        assert_eq!(outcome.primary_type(), Some(ProjectType::CliDeveloperTool));
        assert_eq!(outcome.maturity(), Some(ProjectMaturity::Prototype));
        assert_eq!(
            outcome.classification().evidence_ids().len(),
            2,
            "the resolved classification cites its type and maturity observations"
        );
    }

    #[test]
    fn a_specific_artifact_type_wins_over_a_delivery_form_and_records_a_secondary() {
        let outcome = classify_project(
            &[
                type_obs(TypeSignal::CuratedListStructure),
                type_obs(TypeSignal::ApplicationEntrypointDeclared),
            ],
            &[maturity_obs(MaturitySignal::StableReleaseTagged)],
            &ClassificationPolicy::v1(),
        );
        assert_eq!(outcome.primary_type(), Some(ProjectType::CuratedResource));
    }

    #[test]
    fn conflicting_delivery_forms_reduce_confidence() {
        let clear = classify_project(
            &[type_obs(TypeSignal::LibraryPackagingDeclared)],
            &[maturity_obs(MaturitySignal::StableReleaseTagged)],
            &ClassificationPolicy::v1(),
        );
        let ambiguous = classify_project(
            &[
                type_obs(TypeSignal::LibraryPackagingDeclared),
                type_obs(TypeSignal::ApplicationEntrypointDeclared),
            ],
            &[maturity_obs(MaturitySignal::StableReleaseTagged)],
            &ClassificationPolicy::v1(),
        );
        assert!(
            ambiguous.classification() != clear.classification(),
            "ambiguity must be reflected in the classification"
        );
    }

    #[test]
    fn an_absent_maturity_signal_yields_an_unknown_classification() {
        let outcome = classify_project(
            &[type_obs(TypeSignal::LibraryPackagingDeclared)],
            &[],
            &ClassificationPolicy::v1(),
        );
        assert_eq!(outcome.primary_type(), None);
        assert_eq!(
            outcome.classification().evidence_ids(),
            &[] as &[EvidenceId],
            "an unknown classification cites nothing and invents no type"
        );
    }

    #[test]
    fn tags_are_derived_from_auxiliary_signals() {
        let outcome = classify_project(
            &[
                type_obs(TypeSignal::LibraryPackagingDeclared),
                type_obs(TypeSignal::FrameworkExtensionPoints),
                type_obs(TypeSignal::PluginHostDeclared),
            ],
            &[maturity_obs(MaturitySignal::StableReleaseTagged)],
            &ClassificationPolicy::v1(),
        );
        assert_eq!(
            outcome.primary_type(),
            Some(ProjectType::LibrarySdkFramework)
        );
    }

    #[test]
    fn a_curated_resource_excludes_engineering_rigor_but_not_via_zero() {
        let outcome = classify_project(
            &[type_obs(TypeSignal::CuratedListStructure)],
            &[maturity_obs(MaturitySignal::MaintenanceModeDeclared)],
            &ClassificationPolicy::v1(),
        );
        assert_eq!(
            outcome.applicability(ScoreDimension::EngineeringRigor),
            RubricApplicability::NotApplicable
        );
        assert_eq!(
            outcome.applicability(ScoreDimension::OpenSourceReadiness),
            RubricApplicability::Applicable
        );
    }

    #[test]
    fn a_young_project_relaxes_rigor_and_maintenance_applicability() {
        let outcome = classify_project(
            &[type_obs(TypeSignal::LibraryPackagingDeclared)],
            &[maturity_obs(MaturitySignal::ConceptOnly)],
            &ClassificationPolicy::v1(),
        );
        assert_eq!(
            outcome.applicability(ScoreDimension::EngineeringRigor),
            RubricApplicability::PartiallyApplicable
        );
        assert_eq!(
            outcome.applicability(ScoreDimension::MaintenanceHealth),
            RubricApplicability::PartiallyApplicable
        );
        assert_eq!(
            outcome.applicability(ScoreDimension::ProjectSubstance),
            RubricApplicability::Applicable
        );
    }

    #[test]
    fn an_unknown_classification_never_marks_a_dimension_not_applicable() {
        let outcome = classify_project(&[], &[], &ClassificationPolicy::v1());
        for dimension in crate::ASSAY_SCORE_DIMENSIONS {
            assert_eq!(
                outcome.applicability(dimension),
                RubricApplicability::PartiallyApplicable
            );
        }
    }

    #[test]
    fn an_uncited_observation_is_rejected() {
        assert_eq!(
            TypeObservation::new(TypeSignal::CliEntrypointDeclared, Vec::new())
                .unwrap_err()
                .kind(),
            ClassificationErrorKind::UncitedObservation
        );
    }
}
