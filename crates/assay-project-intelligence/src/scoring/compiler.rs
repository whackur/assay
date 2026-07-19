use std::collections::BTreeMap;

use assay_domain::{EvidenceStatus, RepositorySource, RevisionId, RubricJudgmentSet};

use crate::scoring::classification::ProjectClassification;
use crate::scoring::contribution::{ContributionSource, ScoreContribution};
use crate::scoring::dimensions::{ASSAY_SCORE_DIMENSIONS, ScoreDimension};
use crate::scoring::enums::Visibility;
use crate::scoring::error::{ScoreCompileError, ScoreCompileErrorKind};
use crate::scoring::evaluation::CompiledEvaluation;
use crate::scoring::evaluator::EvaluatorDescriptor;
use crate::scoring::policy::CompilerPolicy;
use crate::scoring::statements::PotentialContext;

pub(crate) const EVALUATION_VERSION: &str = "project-intelligence-1";

/// All inputs required to compile one project evaluation.
pub struct ScoreCompilerInput {
    pub(crate) project_source: RepositorySource,
    pub(crate) revision: RevisionId,
    pub(crate) evaluator: EvaluatorDescriptor,
    pub(crate) visibility: Visibility,
    pub(crate) classification: ProjectClassification,
    pub(crate) deterministic: Vec<crate::scoring::contribution::DeterministicContribution>,
    pub(crate) judgments: Option<RubricJudgmentSet>,
    pub(crate) potential_context: PotentialContext,
    pub(crate) policy: CompilerPolicy,
}

impl ScoreCompilerInput {
    /// Gathers pre-validated inputs for one deterministic compilation.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        project_source: RepositorySource,
        revision: RevisionId,
        evaluator: EvaluatorDescriptor,
        visibility: Visibility,
        classification: ProjectClassification,
        deterministic: Vec<crate::scoring::contribution::DeterministicContribution>,
        judgments: Option<RubricJudgmentSet>,
        potential_context: PotentialContext,
        policy: CompilerPolicy,
    ) -> Self {
        Self {
            project_source,
            revision,
            evaluator,
            visibility,
            classification,
            deterministic,
            judgments,
            potential_context,
            policy,
        }
    }

    /// Compiles the deterministic, versioned project evaluation.
    pub fn compile(&self) -> Result<CompiledEvaluation, ScoreCompileError> {
        let mut grouped: BTreeMap<ScoreDimension, Vec<ScoreContribution>> = BTreeMap::new();
        for contribution in &self.deterministic {
            grouped
                .entry(contribution.dimension())
                .or_default()
                .push(ScoreContribution::new(
                    ContributionSource::DeterministicRule(contribution.rule_id().to_owned()),
                    contribution.applicability(),
                    contribution.value(),
                    contribution.confidence(),
                    contribution.evidence_ids().to_vec(),
                ));
        }

        let judgment_bundle_hash = match &self.judgments {
            Some(set) => {
                if set.evaluation_version().as_str() != EVALUATION_VERSION
                    || set.rubric_version().as_str() != self.evaluator.rubric_version
                {
                    return Err(ScoreCompileError::new(
                        ScoreCompileErrorKind::RubricVersionMismatch,
                    ));
                }
                for judgment in set.judgments() {
                    let dimension = ScoreDimension::from_criterion_prefix(
                        judgment.criterion_id().dimension_prefix(),
                    )
                    .ok_or_else(|| {
                        ScoreCompileError::new(ScoreCompileErrorKind::UnknownCriterionDimension)
                    })?;
                    let normalized = judgment
                        .rating()
                        .map(|rating| f64::from(rating) / f64::from(judgment.rating_scale()));
                    grouped
                        .entry(dimension)
                        .or_default()
                        .push(ScoreContribution::new(
                            ContributionSource::RubricCriterion(judgment.criterion_id().clone()),
                            judgment.applicability(),
                            normalized,
                            judgment.confidence(),
                            judgment.evidence_ids().to_vec(),
                        ));
                }
                Some(set.evidence_bundle_hash().as_str().to_owned())
            }
            None => None,
        };

        let mut dimensions = BTreeMap::new();
        for dimension in ASSAY_SCORE_DIMENSIONS {
            let score = self.compile_dimension(
                dimension,
                grouped.remove(&dimension).unwrap_or_default(),
                self.policy.score_version,
            );
            dimensions.insert(dimension, score);
        }

        let assay_score = self.compile_assay_score(&dimensions);
        let potential = self.compile_potential(
            grouped
                .remove(&ScoreDimension::Potential)
                .unwrap_or_default(),
        );

        let run_status = if assay_score.status == EvidenceStatus::Complete {
            EvidenceStatus::Complete
        } else {
            EvidenceStatus::Partial
        };

        let warnings = self.build_warnings(&assay_score);
        let limitations = self.build_limitations(&assay_score, &dimensions);
        let evidence_ids =
            self.collect_evidence(&dimensions, &assay_score, &potential, &limitations);

        Ok(CompiledEvaluation {
            status: run_status,
            provisional: assay_score.provisional,
            visibility: self.visibility,
            evaluator: self.evaluator.clone(),
            compiler_version: self.policy.compiler_version,
            rule_set_hash: self.policy.rule_set_hash(),
            judgment_bundle_hash,
            project_source: self.project_source.clone(),
            revision: self.revision.clone(),
            classification: self.classification.clone(),
            assay_score,
            dimensions,
            potential,
            evidence_ids,
            warnings,
            limitations,
        })
    }
}
