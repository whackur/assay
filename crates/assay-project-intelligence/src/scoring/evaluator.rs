use serde_json::{Value, json};

use crate::scoring::enums::EvaluatorProvider;
use crate::scoring::error::{ScoreCompileError, ScoreCompileErrorKind};
use crate::scoring::validation::is_version_identifier;

/// Provider-independent evaluator provenance recorded on the evaluation.
#[derive(Clone, Debug, PartialEq)]
pub struct EvaluatorDescriptor {
    pub(crate) profile: String,
    pub(crate) provider: EvaluatorProvider,
    pub(crate) model: Option<String>,
    pub(crate) rubric_version: String,
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

    pub(crate) fn to_value(&self) -> Value {
        json!({
            "profile": self.profile,
            "provider": self.provider.code(),
            "model": self.model,
            "rubric_version": self.rubric_version,
        })
    }
}
