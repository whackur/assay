/// One bounded qualitative project criterion.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct QualitativeCriterion {
    id: &'static str,
    rating_scale: u8,
}

impl QualitativeCriterion {
    /// Returns the stable project-level criterion identifier.
    pub const fn id(&self) -> &'static str {
        self.id
    }

    /// Returns the inclusive upper rating bound; zero is the lower bound.
    pub const fn rating_scale(&self) -> u8 {
        self.rating_scale
    }
}

const PROJECT_V1_CRITERIA: [QualitativeCriterion; 4] = [
    QualitativeCriterion {
        id: "open_source_readiness.coherent_scope",
        rating_scale: 4,
    },
    QualitativeCriterion {
        id: "originality.differentiation",
        rating_scale: 4,
    },
    QualitativeCriterion {
        id: "potential.narrative_credibility",
        rating_scale: 4,
    },
    QualitativeCriterion {
        id: "substance.claim_implementation_fit",
        rating_scale: 4,
    },
];

/// Immutable versioned qualitative rubric understood by all provider adapters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QualitativeRubric {
    version: &'static str,
    evaluation_version: &'static str,
    criteria: &'static [QualitativeCriterion],
}

impl QualitativeRubric {
    /// Returns the initial project rubric from the Project Intelligence spec.
    pub const fn project_v1() -> Self {
        Self {
            version: "project-rubric-1",
            evaluation_version: "project-intelligence-1",
            criteria: &PROJECT_V1_CRITERIA,
        }
    }

    /// Returns the version recorded in provider output and snapshots.
    pub const fn version(&self) -> &'static str {
        self.version
    }

    /// Returns the Project Intelligence evaluation-version boundary.
    pub const fn evaluation_version(&self) -> &'static str {
        self.evaluation_version
    }

    /// Returns criteria in canonical identifier order.
    pub const fn criteria(&self) -> &'static [QualitativeCriterion] {
        self.criteria
    }

    pub(crate) fn criterion(&self, id: &str) -> Option<&'static QualitativeCriterion> {
        self.criteria.iter().find(|criterion| criterion.id == id)
    }
}
