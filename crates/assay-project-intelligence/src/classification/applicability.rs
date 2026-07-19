use std::collections::BTreeMap;

use assay_domain::RubricApplicability;

use crate::ProjectMaturity;
use crate::ProjectType;
use crate::ScoreDimension;

/// Resolves type- and maturity-specific criteria applicability.
///
/// The base applicability reflects each type's evaluable surface; maturity only
/// relaxes an `Applicable` dimension to `PartiallyApplicable` for young or
/// end-of-life projects, never tightening it, so a young project is never
/// penalized for absent long-term evidence.
pub fn criteria_applicability(
    primary_type: ProjectType,
    maturity: ProjectMaturity,
    _policy: &crate::classification::policy::ClassificationPolicy,
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

pub(crate) fn unknown_applicability() -> BTreeMap<ScoreDimension, RubricApplicability> {
    crate::ASSAY_SCORE_DIMENSIONS
        .into_iter()
        .map(|dimension| (dimension, RubricApplicability::PartiallyApplicable))
        .collect()
}
