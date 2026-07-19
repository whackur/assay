use std::collections::BTreeMap;

use assay_domain::{EvidenceId, EvidenceStatus, RepositorySource, RevisionId};
use serde_json::{Value, json};

use crate::scoring::classification::ProjectClassification;
use crate::scoring::compiler::EVALUATION_VERSION;
use crate::scoring::dimensions::ScoreDimension;
use crate::scoring::enums::Visibility;
use crate::scoring::evaluator::EvaluatorDescriptor;
use crate::scoring::mapping::{
    SCHEMA_VERSION, diagnostics, evidence_values, repository_value, status_code,
};
use crate::scoring::scores::{AssayScore, DimensionScore, PotentialScore};

/// A compiled, dimensioned project evaluation with a public machine mapping.
#[derive(Clone, Debug, PartialEq)]
pub struct CompiledEvaluation {
    pub(crate) status: EvidenceStatus,
    pub(crate) provisional: bool,
    pub(crate) visibility: Visibility,
    pub(crate) evaluator: EvaluatorDescriptor,
    pub(crate) compiler_version: &'static str,
    pub(crate) rule_set_hash: String,
    pub(crate) judgment_bundle_hash: Option<String>,
    pub(crate) project_source: RepositorySource,
    pub(crate) revision: RevisionId,
    pub(crate) classification: ProjectClassification,
    pub(crate) assay_score: AssayScore,
    pub(crate) dimensions: BTreeMap<ScoreDimension, DimensionScore>,
    pub(crate) potential: PotentialScore,
    pub(crate) evidence_ids: Vec<EvidenceId>,
    pub(crate) warnings: Vec<(String, Vec<EvidenceId>)>,
    pub(crate) limitations: Vec<(String, Vec<EvidenceId>)>,
}

impl CompiledEvaluation {
    /// Returns the overall Assay Score.
    pub const fn assay_score(&self) -> &AssayScore {
        &self.assay_score
    }

    /// Returns one compiled Assay Score dimension.
    pub fn dimension(&self, dimension: ScoreDimension) -> Option<&DimensionScore> {
        self.dimensions.get(&dimension)
    }

    /// Returns the separate Potential indicator.
    pub const fn potential(&self) -> &PotentialScore {
        &self.potential
    }

    /// Returns whether the Assay Score is a provisional normalization.
    pub const fn provisional(&self) -> bool {
        self.provisional
    }

    /// Maps the evaluation onto `schemas/project-evaluation/v1.json`.
    pub fn to_machine_value(&self) -> Value {
        let scores = json!({
            "assay_score": self.assay_score.to_value(),
            "project_substance": self.dimensions[&ScoreDimension::ProjectSubstance].to_value(),
            "originality": self.dimensions[&ScoreDimension::Originality].to_value(),
            "engineering_rigor": self.dimensions[&ScoreDimension::EngineeringRigor].to_value(),
            "open_source_readiness": self.dimensions[&ScoreDimension::OpenSourceReadiness].to_value(),
            "maintenance_health": self.dimensions[&ScoreDimension::MaintenanceHealth].to_value(),
            "potential": self.potential.to_value(),
        });
        json!({
            "schema_version": SCHEMA_VERSION,
            "evaluation_version": EVALUATION_VERSION,
            "status": status_code(self.status),
            "provisional": self.provisional,
            "visibility": self.visibility.code(),
            "evaluator": self.evaluator.to_value(),
            "compiler": {
                "version": self.compiler_version,
                "rule_set_hash": self.rule_set_hash,
                "judgment_bundle_hash": self.judgment_bundle_hash,
            },
            "project": {
                "source": repository_value(&self.project_source),
                "revision": self.revision.as_str(),
            },
            "classification": self.classification.to_value(),
            "scores": scores,
            "evidence_ids": evidence_values(&self.evidence_ids),
            "introduction": {
                "status": status_code(EvidenceStatus::Unavailable),
                "factual_statements": [],
                "interpretations": [],
            },
            "warnings": diagnostics(&self.warnings),
            "limitations": diagnostics(&self.limitations),
        })
    }
}
