use sha2::{Digest, Sha256};

use crate::scoring::dimensions::{DIMENSION_WEIGHTS, ESSENTIAL_DIMENSIONS, ScoreDimension};

pub(crate) const RULE_SET_DOMAIN: &[u8] = b"assay.project-intelligence.score-compiler.rule-set.v1";

/// Versioned weight, sufficiency, applicability, and forecast policy.
///
/// Every field is versioned data folded into the published rule-set hash, so a
/// weight or rule change is visible rather than a silent constant edit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerPolicy {
    pub(crate) compiler_version: &'static str,
    pub(crate) score_version: &'static str,
    pub(crate) potential_version: &'static str,
    pub(crate) weight_policy_version: &'static str,
    pub(crate) sufficiency_policy_version: &'static str,
    pub(crate) applicability_policy_version: &'static str,
    pub(crate) forecast_policy_version: &'static str,
    pub(crate) forecast_horizon: &'static str,
    pub(crate) partial_weight_basis_points: u32,
    pub(crate) provisional_confidence_basis_points: u32,
}

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

    pub(crate) fn weight(&self, dimension: ScoreDimension) -> f64 {
        DIMENSION_WEIGHTS
            .into_iter()
            .find_map(|(candidate, weight)| (candidate == dimension).then_some(f64::from(weight)))
            .unwrap_or(0.0)
    }

    pub(crate) fn is_essential(&self, dimension: ScoreDimension) -> bool {
        ESSENTIAL_DIMENSIONS.contains(&dimension)
    }

    pub(crate) fn partial_weight(&self) -> f64 {
        f64::from(self.partial_weight_basis_points) / 10_000.0
    }

    pub(crate) fn provisional_penalty(&self) -> f64 {
        f64::from(self.provisional_confidence_basis_points) / 10_000.0
    }

    pub(crate) fn rule_set_hash(&self) -> String {
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
            crate::scoring::compiler::EVALUATION_VERSION,
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
