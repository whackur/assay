use std::collections::BTreeMap;

use assay_domain::RubricApplicability;

use crate::ProjectClassification;
use crate::ProjectMaturity;
use crate::ProjectType;
use crate::ScoreDimension;

/// The resolved classification with its aligned criteria applicability.
#[derive(Clone, Debug, PartialEq)]
pub struct ClassificationOutcome {
    pub(crate) classification: ProjectClassification,
    pub(crate) primary_type: Option<ProjectType>,
    pub(crate) maturity: Option<ProjectMaturity>,
    pub(crate) applicability: BTreeMap<ScoreDimension, RubricApplicability>,
    pub(crate) policy_version: &'static str,
    pub(crate) applicability_policy_version: &'static str,
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
