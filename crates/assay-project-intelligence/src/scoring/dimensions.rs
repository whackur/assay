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

pub(crate) const DIMENSION_WEIGHTS: [(ScoreDimension, u32); 5] = [
    (ScoreDimension::ProjectSubstance, 25),
    (ScoreDimension::Originality, 20),
    (ScoreDimension::EngineeringRigor, 25),
    (ScoreDimension::OpenSourceReadiness, 15),
    (ScoreDimension::MaintenanceHealth, 15),
];

pub(crate) const ESSENTIAL_DIMENSIONS: [ScoreDimension; 3] = [
    ScoreDimension::ProjectSubstance,
    ScoreDimension::EngineeringRigor,
    ScoreDimension::OpenSourceReadiness,
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

    pub(crate) const fn criterion_prefix(self) -> &'static str {
        match self {
            Self::ProjectSubstance => "substance",
            Self::Originality => "originality",
            Self::EngineeringRigor => "engineering_rigor",
            Self::OpenSourceReadiness => "open_source_readiness",
            Self::MaintenanceHealth => "maintenance_health",
            Self::Potential => "potential",
        }
    }

    pub(crate) fn from_criterion_prefix(prefix: &str) -> Option<Self> {
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
