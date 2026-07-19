/// Versioned classification confidence and applicability policy.
///
/// Every field is versioned data recorded on the outcome so a rule change is
/// visible rather than a silent constant edit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClassificationPolicy {
    pub(crate) policy_version: &'static str,
    pub(crate) applicability_policy_version: &'static str,
    pub(crate) single_type_confidence_bp: u32,
    pub(crate) ambiguous_type_confidence_bp: u32,
    pub(crate) maturity_confidence_bp: u32,
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
