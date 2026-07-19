/// Versioned facet weights and cohort-size policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComparisonPolicy {
    pub(crate) comparison_version: &'static str,
    pub(crate) detailed_limit: usize,
    pub(crate) default_facet_weight: u32,
}

pub(crate) const COMPARISON_VERSION: &str = "project-comparison-1";

impl ComparisonPolicy {
    /// Returns the initial versioned comparison policy.
    pub const fn v1() -> Self {
        Self {
            comparison_version: COMPARISON_VERSION,
            detailed_limit: 5,
            default_facet_weight: 10,
        }
    }

    pub(crate) fn facet_weight(&self, facet: &str) -> u32 {
        match facet {
            "problem_overlap" | "feature_overlap" | "entry_overlap" | "list_structure" => 30,
            "technical_similarity"
            | "structural_similarity"
            | "unique_coverage"
            | "editorial_quality"
            | "maintenance_evidence" => 20,
            _ => self.default_facet_weight,
        }
    }
}
