use serde::{Deserialize, Serialize};

/// Applicability of a rubric criterion or deterministic check to the classified
/// project. `NotApplicable` is an explicit status, never a zero score.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RubricApplicability {
    Applicable,
    PartiallyApplicable,
    NotApplicable,
}
